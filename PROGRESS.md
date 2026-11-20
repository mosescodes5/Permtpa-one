# PROGRESS LOG — PERMTPA ONE

## Milestone 4 · Step 2 — Business-type onboarding + deletable sample catalogue: **WRITTEN, PARTLY VERIFIED (web only)**
Authored in a sandbox with **no Rust, PostgreSQL or Redis** (no `cargo`, `psql`, `redis-server`). Node was available, so the web side was actually run.

### State recorded at the start of this step
- Step 1 (migration 0009): the developer reported it **green in CI and working by hand in Codespaces**. The three CI results (Playwright pass/skip counts from the `e2e` job, the last line of `scripts/test-db.sh`, the `test-0009-upgrade.sh` result lines) have **not yet been pasted**, so **no Step 1 counts are recorded here**. The five Step 1 mutation checks are still owed.

### Delivered (UNVERIFIED except where stated)
- **Migration 0010** (`0010_business_types_and_samples.sql`):
  - `business_types` gains description, sort order, `selectable`, and the capability flags `tracks_expiry`, `sold_by_weight`, `has_tables`, `has_bookings`. 12 types are offered; the four older codes (retail, agriculture, distribution, agency) stay valid for existing organizations but are hidden. Capabilities are read through the organization's type (join), so they cannot drift; the app role has SELECT only.
  - `organizations.phone` (optional, shape-checked; owner/administrator only via existing RLS).
  - Sample flags: `products.sample_key`, `stock_movements.is_sample`. The app role cannot create them (RLS `WITH CHECK` on products and movements) or change them (no UPDATE grant).
  - `sample_catalogue_items` (61 rows, 12 types; holds cost prices, so no grant and RLS with no policy). Data checked by a generator script against the same rules the table enforces.
  - `app.load_sample_catalogue`, `app.delete_sample_catalogue`, `app.sample_catalogue_status` (SECURITY DEFINER, pinned search_path, role and tenant checked inside the function).
  - **Append-only exception (needs your review):** `stock_movements` is still append-only except that a row flagged `is_sample` may be deleted while `delete_sample_catalogue()` runs (transaction-local flag `app.sample_purge`). The app role has no DELETE privilege at all, so the flag is useless to it; real movements can never be deleted.
- **API** (`src/onboarding.rs`): `GET /v1/business-types`, `GET|POST|DELETE /v1/orgs/{id}/sample-catalogue`; org profile now returns `phone` and `capabilities`; `PATCH /v1/orgs/{id}` accepts `phone` (empty clears); products return `isSample`.
- **Web**: onboarding screen with 12 cards, optional phone, sample checkbox, "Finish setup"; Organization page uses the API list and edits the phone; Inventory shows a "Sample" badge, inline confirm before deleting, "Load sample catalogue" from the empty state.
- **Behaviour actually implemented from capability flags in this step:** `sold_by_weight` types get fractional sample products. `tracks_expiry`, `has_tables`, `has_bookings` are **stored and exposed only**; the expiry, tables and bookings features do not exist yet.

### Verified here (actually run in the sandbox)
- Web: `tsc --noEmit` clean, `eslint` clean, Vitest **54 passed (6 files)**, including 7 new onboarding-form tests and 3 new schema tests.
- Sample data generator: all 61 rows satisfy the key/SKU/price/whole-unit rules.

### NOT verified
Everything Rust and SQL: `cargo fmt/clippy/test`, migration 0010 on a real database, the SQL attack suite additions, `tests/onboarding.rs` (12 DB-backed tests), Playwright (new `onboarding.spec.ts` and the changed `createWorkspace` helper), Docker, real SMTP, Windows, mobile browsers. Run `cargo fmt --all` first: the Rust was written without rustfmt.

### Mutation checks owed for Step 2 (throwaway branch + draft PR; expect RED)
1. Remove `and sample_key is null` from the `products_insert` policy: the SQL suite "cannot create a product with a sample_key" must fail.
2. Remove `and is_sample = false` from `movements_insert`: "cannot create a flagged stock movement" must fail.
3. In `stock_movements_immutable()`, drop the `old.is_sample` condition: the trigger test and SQL "real movements" assertions must fail.
4. In `delete_sample_catalogue`, drop the `m.is_sample = false` history check: the "kept with real history" tests must fail.
5. In `load_sample_catalogue`, remove the `p_org is distinct from app.current_org_id()` test: cross-tenant SQL tests must fail.
6. Remove `on conflict ... do nothing` handling or the `sample_key` unique index: `concurrent_loads_never_duplicate` must fail (may need several runs).
7. `grant select on sample_catalogue_items to permtpa_app`: the cost-leak assertions must fail.

### Known gaps
- Service-type businesses (salon, car wash, services, hotel) get sample *goods*; a "service with no stock" item type does not exist yet.
- Sample products have no categories (avoids leftover empty categories on delete).
- A kept sample (it has real history) stays flagged "Sample"; there is no "keep as a normal product" action yet.
- Legacy business-type codes are still accepted by the API and database (many existing tests use them); they are only hidden from the picker.
- The add-product form does not yet default "allow decimals" from `soldByWeight`.
- Your list says 11 business types but names 12; 12 are implemented.

## Milestone 4 · Step 1 — Fractional quantities: **WRITTEN, NOT VERIFIED**
Authored in a sandbox with no Rust, PostgreSQL, Redis or network. **Nothing below has been compiled or run.** The developer verifies in VS Code / CI (`.github/workflows/ci.yml`, plus the new `scripts/test-0009-upgrade.sh` step).
The Milestone 3 baseline (Vitest 35, Rust 30, DB 13/9/12/3/2, SQL 126, Playwright 29+7 skipped) was NOT re-verified; treat as unverified claims.

### Changes
- **Migration 0009** (`services/api/migrations/0009_fractional_quantities.sql`): your draft plus (a) a `qty_scale: 1000` marker on new `stock.adjust` audit rows (rows written before 0009 have no marker and hold whole-unit deltas); (b) `allow_fractional` is settable only at INSERT, never UPDATE (column grant unchanged).
- **API** (`src/inventory.rs`, `src/team.rs`): quantities `i64` thousandths, `MAX_QTY` 100_000_000_000; `allowFractional` on product create and in responses; whole-unit products reject fractional opening stock / reorder level in the API (`fractional_not_allowed`) and fractional movements in the DB trigger (mapped to the same 400); valuation = round(Σ on_hand×cost ÷ 1000) once, half away from zero; response key `units` renamed `unitsThousandths`; `i64::MIN` delta no longer overflows `abs()`.
- **Rust tests** (`tests/inventory.rs`): existing references rescaled; new tests for exact 2.5 kg arithmetic, valuation rounding (1.750 kg × 1001 = 1752), whole-unit refusals on every entry path, `allowFractional` not updatable, 25-way concurrent 0.4 kg withdrawals on 2.0 kg (expect exactly 5). Unit tests added for `whole_unit_ok` and bounds.
- **SQL attack suite** (`tests/db/rls_attacks.sql`): existing assertions rescaled (the `sale`-reserved test now uses a whole-unit delta so it cannot pass for the wrong reason); new attacks: fractional accepted/exact on fractional product, whole-unit product rejects +0.5/+1.5/-0.001, app role cannot flip `allow_fractional` either way, delta/reorder caps, cross-tenant fractional movement, audit scale marker.
- **Upgrade test** (`scripts/test-0009-upgrade.sh`, `tests/db/upgrade_0009_*.sql`): migrates to 0008, seeds whole-unit data, applies 0009, asserts balances, reorder levels, deltas and resulting balances were multiplied by 1000 and the old audit row is untouched.
- **Web**: `src/lib/quantity.ts` (string-based parse/format, no floats) with `tests/unit/quantity.test.ts`; add-product form has an "allows decimals" checkbox; stock, history, edit, inventory KPI and dashboard use it; e2e seed values rescaled.

### NOT verified (all of it)
cargo build/test/clippy/fmt, SQLx compile-time checks, migration 0009 on a real database (constraint names assume Postgres defaults), SQL suite, upgrade test, Vitest/ESLint/tsc, Playwright, Docker, cargo audit/deny, real SMTP, Windows. **Mutation checks of the new controls have not been run**; do them after CI is green (drop the trigger's `% 1000` check, grant UPDATE(allow_fractional), remove the API whole-unit check, and confirm the matching tests fail).
Also unconfirmed: any other code that reads quantity columns as i32 (grep `i32` and `on_hand` across `services/api/src` besides inventory.rs).

### Next
Step 2: business-type onboarding with a deletable sample catalogue.

---

## Milestone 3 · Part B — Inventory: **COMPLETE (verified in sandbox)**
Milestone 3 is now complete (3A teams + 3B inventory). Built for a **retail shop** with **whole-number quantities** (an assumption — no business type was chosen).

### Delivered
- **DB (migration 0008):** categories, products, variants (SKU/barcode unique *per organization*), variant costs (separate table so RLS can hide them), `stock_levels` (read-only to the app) and append-only `stock_movements`. A trigger is the only way a balance changes: it locks the row, refuses negative stock and records the balance after each movement. Composite foreign keys make cross-tenant references impossible. Cost prices are invisible to Sales Staff and Read Only *at the database level*. Audit triggers for product/variant/cost/price changes and stock adjustments.
- **API:** categories, products (with variants, costs and opening stock in one transaction), variant edits, stock adjustments (reason + note rules, sign rules, `insufficient_stock` → 409), movement history (keyset pagination), barcode/SKU lookup, low-stock list, valuation (exact `NUMERIC` arithmetic, returned as a string). Search with escaped wildcards, capped pagination, archive instead of delete. Domain crate gained `can_manage_inventory` / `can_view_costs` with a **drift-guard test** that parses the SQL role lists.
- **Web:** Inventory page (KPIs, search, category and status filters, pagination, stock badges), dialogs for add product / adjust stock / history / edit prices / categories, exact naira parsing (no floats), live low-stock card on the dashboard, role-aware UI.

### Verification (final fresh run)
| Check | Result |
|---|---|
| ESLint / tsc | 0 problems / clean |
| Vitest | **35 passed** |
| `cargo test --workspace` (non-DB) | **30 passed** |
| DB-backed Rust: auth / team / **inventory** / tenancy / smtp | **13 / 9 / 12 / 3 / 2 passed** |
| SQL adversarial suite | **126 passed, 0 failed** (43 new inventory assertions) |
| Full-stack Playwright (desktop + mobile) | **29 passed, 7 skipped, 0 failed** |
| `npm audit --omit=dev` | 0 vulnerabilities |
| Mutation tests — inventory SQL | **14/14 caught** |
| Mutation tests — inventory API | **15/15 caught** (after fixing one weak test, below) |
| Concurrency | 25 simultaneous withdrawals against 10 units → exactly 10 succeed, final balance 0, history sum = balance |
| Valuation exactness | 10^12 × 10^8 = 10^20 (beyond 64-bit) returned exactly |
Visually reviewed: inventory page (dark), adjust-stock dialog; stock value ₦972K matched a hand calculation.

### Defects found and fixed this round (for honesty)
1. **Variant order was random** (variants created in one transaction share `now()`; ties were broken by random UUID). Caught by a flaky run; fixed with an identity `seq` column; a regression test is proven to fail on the old code.
2. **Weak test found by mutation testing:** the page-size cap was only checked via the *echoed* number. Limit is now single-sourced and the test loads 113 rows and checks only 100 return.
3. **Mobile layout bug:** screen-reader-only text (`position:absolute`) inside a scrolling table escaped its clip and stretched the page to 675px on a 412px phone. All scroll containers are now positioned. The first mobile "pass" was a **false pass** (the test measured a loading skeleton); the test now waits for real content and is proven to fail when the bug is re-introduced.
4. **Process errors:** the sandbox disk filled to 100% (linker failed — cleared build caches); a mutation run exceeded the time limit and left a source file mutated (detected and restored; runs are now batched, gated and restore-verified).

### Known issues / limitations
- Quantities are whole numbers only (no kg/litre decimals); no expiry dates, batches, serial numbers, multiple locations or stock transfers.
- Valuation = current cost × on-hand (not FIFO/weighted-average). Changing a cost reprices all stock.
- UI gaps (API exists): editing product name/category, adding a variant to an existing product, bulk import, camera barcode scanning.
- Concurrency was tested with parallel requests in one API process, not across multiple API instances (the row lock is in the database, so it should hold, but this is untested).
- Purchase orders and suppliers belong to Milestone 4; the `sale` movement reason is reserved for the sales module.
- Windows, Docker, `clippy`, `cargo audit`/`deny` remain unverified in this sandbox.

### Next: Milestone 4 — Sales / POS, customers, suppliers
Sales with multiple lines, discounts, tax, payment methods, receipts, refunds/returns (stock-consistent via the reserved `sale`/`return_in` movements), customers with balances, suppliers and purchase orders.

---

## Milestone 3 · Part A — Teams & organizations: **COMPLETE (verified in sandbox)**
Part B (products / inventory) is **not started**.

### Delivered
- **DB (migration 0007):** `app.accept_invitation()` (caller's *verified* email must equal the invited email; atomic single use), one-pending-invitation-per-email index, automatic audit triggers for membership changes / invitations / org edits (actor comes from the transaction context, never the client).
- **API:** org profile (GET/PATCH), invitations (create, list, revoke, accept), member role/status change and removal. Roles enforced twice (explicit checks + RLS); DB rejections map to 403 / 409 `last_owner`; non-members get 404; `deny_unknown_fields` everywhere; invitation tokens stored hashed and never returned by the API; owners can never be granted by invitation.
- **Web:** Team page (members, role selects, suspend/reinstate, confirm-to-remove, invite form, pending invites), Organization page, accept-invite flow (token survives the login redirect; invitees are not forced into onboarding), role-aware UI (Managers etc. see a read-only list without colleagues' emails).

### Verification (final fresh run)
| Check | Result |
|---|---|
| ESLint / tsc | 0 problems / clean |
| Vitest | **32 passed** |
| `cargo test --workspace` (non-DB) | **27 passed** |
| HTTP suites (real PG + Redis): auth_flow / **team** / tenancy / smtp | **13 / 9 / 3 / 2 passed** |
| SQL adversarial suite | **83 passed, 0 failed** |
| Full-stack Playwright (desktop + mobile) | **25 passed, 5 skipped, 0 failed** (incl. 3 new team specs) |
| `npm audit --omit=dev` | 0 vulnerabilities |
| Mutation tests — team API | 8 caught directly; 2 are **equivalent mutants** because the database enforces the same rule (proven: removing the API check *and* the RLS policy together is caught) |
| Mutation tests — new SQL | 2/2 caught (invitation email binding, audit trigger) |

### Mistakes made and fixed this round (for honesty)
1. **My first mutation run was INVALID.** `team.rs` was not yet committed, so the script's `git checkout` restore silently did nothing and mutations accumulated; the "CAUGHT" lines were just a broken baseline failing. I detected it when the baseline itself failed, restored the file from source, committed, added a baseline gate + restore-verification to the script, and re-ran everything. Only the re-run results above are valid.
2. **Wrong claim from M2 corrected** (see the TRUSTED_PROXY_HOPS note above); it surfaced as 3 flaky mobile e2e failures caused by a shared rate-limit bucket (13 registrations vs a limit of 10/hour).
3. Windows: the e2e API start command used a Unix path and failed on `cmd.exe`; now `cargo run -p permtpa-api` with `cwd` (verified on Linux only — **not tested on Windows**).
4. A stray Redis `dump.rdb` was committed; removed and git-ignored. UI copy for a wrong-account invitation was too generic; fixed.

### Known issues / not verified
- Windows: `scripts/e2e-setup.sh` needs Git Bash or WSL; Postgres, Redis and Rust must be installed. Only Linux was tested.
- No ownership-transfer UI beyond changing roles; no "leave organization" for non-admins; no invitation resend button (re-inviting replaces the old link).
- Invitation emails use the same SMTP path (verified only against a local plaintext sink).
- Docker, `clippy`, `rustfmt --check`, `cargo audit`, `cargo deny` still not run in this sandbox.

### Next
**Milestone 3 · Part B — Inventory:** categories, products, variants, SKUs, barcodes, append-only stock movements, adjustments with reasons, reorder thresholds, valuation (cost prices visible only to finance-capable roles), and real data for the dashboard's low-stock card.

---

## UI refresh — animated icon set & motion (post-M2): **COMPLETE (verified in sandbox)**
- **Icons:** removed Lucide entirely (dependency uninstalled). New original duotone icon set in `apps/web/src/components/icons` — 50 icons, each with its own hover animation (cart rolls, box lid lifts, bars regrow, bell swings, gear turns, shield check redraws, lock shackle lifts, etc.). Icons react when their nearest `a`/`button`/`[data-hover-scope]` ancestor is hovered or focused, so no call-site changes were needed.
- **Motion:** hero word-stagger, floating product preview, drifting gradient glow, chart wipe-in, count-up KPIs (also ease between values when the date filter changes), industries marquee (pauses on hover), card lift + icon-tile fill on hover, shine sweep on primary buttons, animated link underlines, hamburger→X morph, sliding sidebar active indicator, pop-in menus / command palette, route transitions, growing progress bars.
- **Accessibility:** `<MotionConfig reducedMotion="user">` + global CSS rule; server HTML already contains final numbers/text (no JS needed to read content).
- **Verification:** eslint 0 problems, tsc clean, Vitest 31 passed, full-stack Playwright **22 passed, 2 skipped** (new tests: card hover effect, reduced-motion behaviour). Visually reviewed an icon gallery, hero, and a mid-hover frame.
- **Bug caught visually:** hero headline words rendered without spaces (trailing space inside `inline-block` collapses); fixed by placing spaces between the animated spans.
- **Not verified:** animations were checked in Chromium only (no Safari/Firefox); no performance profiling on low-end phones; the dashboard in dark mode was not re-screenshotted after the refresh (covered by e2e only).

---

## Milestone 2 — Authentication, sessions, MFA, tenancy wiring: **COMPLETE (verified in sandbox)**

### Delivered
- **API** (`/v1`): register, verify-email, resend-verification, login (+ optional `mfaCode`), logout, forgot/reset-password, csrf, `me`, session list / revoke / revoke-others, MFA setup / enable / disable, create organization, list org members (role-aware: only Owner/Admin see emails). RFC 9457 problem+json errors.
- **Security**: Argon2id; opaque hashed session tokens in HttpOnly cookies with rotation, idle + absolute expiry; Origin + session-bound CSRF; Redis rate limits (fail-closed) + progressive lockout; enumeration-safe responses; single-use purpose-bound email tokens; TOTP with replay protection + hashed single-use recovery codes; AES-256-GCM for MFA secrets; `deny_unknown_fields` everywhere; SMTP transport with TLS enforced outside development; trusted-proxy-aware client IP.
- **Database**: migrations 0005 (tokens, lockout accounting, verified-only session resolution) and 0006 (MFA). Audit events for register / verify / login / failed login / reset / MFA.
- **Web**: forms wired to the real API (no placeholders), open-redirect-safe `?next=`, verify-email, reset-password, onboarding (create workspace), real user/org in the shell, working logout, org switcher from real memberships, Settings page (profile, MFA with QR + recovery codes, session management), same-origin `/api` proxy.

### Verification results (final fresh run, M2)
| Check | Result |
|---|---|
| ESLint / tsc strict | 0 problems / clean |
| Vitest | 4 files, **31 passed** |
| `cargo test --workspace` (non-DB) | **27 passed** (18 DB/SMTP-backed tests marked ignored, run below) |
| HTTP auth integration suite (real PG + Redis, `--ignored`) | **13 passed** (stable over 3 runs at 6-way parallelism) |
| Tenancy via Rust path | **3 passed** |
| SMTP delivery to a real local SMTP server | **2 passed** (delivery with intact link; CRLF header injection refused) |
| SQL adversarial RLS/auth suite | **67 passed, 0 failed** |
| Full-stack Playwright (Next → Rust API → PG + Redis; desktop + mobile Chromium) | **19 passed, 1 skipped** (MFA flow is desktop-only by design) |
| `npm audit --omit=dev` | 0 vulnerabilities |
| Mutation testing — Rust security controls | **12/12 detected** (CSRF token, Origin, lockout, TOTP replay*, recovery-code burn, IDOR 404, rate limit, session rotation, mass assignment, reset-revokes-sessions, dev outbox in prod, enumeration) |
| Mutation testing — SQL | **7/7 detected** after fixing one vacuous test (see below) |
\* TOTP replay is enforced in two layers; removing one is an equivalent mutant, removing both is detected.

### Defects found and fixed during M2 (for honesty)
1. **Real bug (caught by unit test):** `loginSchema` did not declare `mfaCode`; Zod stripped it, so MFA users could never log in via the UI. Fixed.
2. **Real UX bug (caught by e2e):** after explicit logout the auth gate raced the redirect and produced `/login?next=/dashboard`. Fixed.
3. **Vacuous test (caught by mutation testing):** the SQL "unverified user cannot resolve a session" assertion looked up an email-token hash, not a session, and could never fail. Rewritten with a real session; mutant now detected.
4. Test-harness races (parallel `ALTER ROLE`) serialised; stale fixtures and a convoluted selector fixed.
5. Config validation order made deterministic (DB/Redis URLs before secrets).

### Known issues / not verified (M2)
- **Docker still not run** (unavailable in sandbox). `clippy`, `rustfmt --check`, `cargo audit`, `cargo deny` not run (components unavailable); wired into CI.
- **SMTP verified only against a plaintext local sink.** TLS delivery to a real provider is untested. Without `SMTP_URL`, non-development environments refuse to send (loudly) rather than pretend.
- Argon2 parameters are library defaults, not benchmarked for production hardware. No breached-password corpus check; no CAPTCHA.
- Not implemented yet: logged-in password change, email change, recovery-code regeneration, account deletion, invitations (M3).
- UI route guard is client-side UX only; real enforcement is in the API/DB. Web CSP is not yet enforced (M8).
- Redis connections are opened per rate-limit call (correct but not optimised).
- `TRUSTED_PROXY_HOPS` must match deployment topology (1 behind the Next proxy). **Correction (found in M3A):** an earlier note claimed e2e used 2 hops so tests could present unique IPs. That was wrong — Next replaces `X-Forwarded-For` with the socket peer, so all e2e traffic shares one rate-limit bucket. E2E now uses 1 hop (production-like) and a development-only `/v1/dev/reset-rate-limits` endpoint.
- E2E in the sandbox uses an npm-distributed Chromium via `PW_CHROMIUM_PATH`; CI uses the standard Playwright browsers.

### Next: Milestone 3 (not started)
Organization management (profile, invitations → accept flow, role assignment UI, member management), products / categories / variants / SKUs / barcodes, stock movements and adjustments, reorder thresholds, inventory valuation. Each new tenant table gets `org_id` + RLS + attack tests from day one.

---

## Milestone 1 — Foundation: **COMPLETE (verified in sandbox)**

### Delivered
- Monorepo (npm workspaces + Cargo workspace), git initialised.
- **Web**: Next.js 16.4.6 / React 19.3.1 / Tailwind 4.3.5 / TypeScript 6.0.3 (strict + `noUncheckedIndexedAccess`) / ESLint 9.39.5. Design system tokens (light/dark), primitives, logo, theming.
  Public: landing (hero w/ product preview, platform, Intelligence, security, pricing w/ monthly/annual toggle, CTA), About/Contact/Privacy/Terms (legal pages are labelled draft placeholders).
  Auth UI: login, register, forgot-password (RHF + Zod; honest "service not connected" state — no fake login).
  App shell: collapsible sidebar, mobile drawer, org switcher, notifications, user menu, theme toggle, command palette (Ctrl/⌘+K), `[` shortcut, skip link; dashboard (KPIs, area chart, date filter, low stock, tables) clearly labelled **sample data**; roadmap placeholders for unbuilt modules.
- **Database**: 4 migrations — identity, tenancy, RLS, SECURITY DEFINER auth functions, immutable audit log, last-owner guard, column-level grants.
- **Rust**: Axum 0.8.9 / sqlx 0.9.0 / tower-http 0.7.1 / redis 1.8.1 / tokio 1.54.1 on Rust 1.97.1. `/healthz`, `/readyz`, request IDs, security headers, body limit, graceful shutdown, `migrate` subcommand, tenant-scoped transaction helper, redacting config.
- **Infra**: `docker-compose.yml` (Postgres 17 + Redis 7, loopback-only), `.env.example` (placeholders), role-bootstrap script (idempotent, rotates credentials), GitHub Actions CI, `deny.toml`.

### Verification results (final fresh run)
| Check | Result |
|---|---|
| `eslint .` | 0 problems |
| `tsc --noEmit` | clean |
| Vitest | 3 files, **20 passed** |
| `next build` | success, 23 static pages |
| Playwright (desktop + mobile Chromium) | **12 passed** |
| `cargo test --workspace` | **12 passed**, 3 ignored (DB-backed; run separately) |
| Rust DB-backed tenancy tests (`--ignored`) | **3 passed** against PostgreSQL 16.15 |
| SQL adversarial RLS suite | **42 passed, 0 failed** |
| Mutation checks (5 deliberate sabotages) | **5/5 detected** by the suite |
| Live API run | `/healthz` 200; `/readyz` 200 → **503** when Redis stopped; graceful shutdown OK; 0 secrets in logs |
| `npm audit --omit=dev` | 0 vulnerabilities (8 moderate, dev-tooling only: `minimatch` via ESLint) |

### Defects found and fixed during the build (for honesty)
1. Test helper `blocked()` swallowed its own failure (same SQLSTATE as the exceptions it catches) → falsely passed. Found by mutation testing; fixed; suite re-verified.
2. Init-script password interpolation bug (literal `${...}` set as the password) → caught before use; fixed.
3. Lint: effects that set state synchronously → replaced with `useSyncExternalStore`, remount-on-open and `useWatch`.
4. Playwright's browser CDN is blocked in the sandbox; used an npm-distributed Chromium via `PW_CHROMIUM_PATH` (documented, optional).

### Known issues / not verified
- **Docker was not available**: `docker-compose.yml` is YAML-validated only, never started. The same Postgres init script and migrations *were* run against a local PostgreSQL 16; Compose pins Postgres 17 (untested).
- `cargo clippy`, `cargo fmt --check`, `cargo audit`, `cargo deny` were **not run** (components/network unavailable); they are wired into CI.
- Sidebar order/org list/user are **sample data**. No real authentication yet.
- TypeScript 7.0.2 and ESLint 10 exist on npm but were not adopted (npm resolved the Next-16-compatible 6.0.3 / 9.39.5).
- Annual price = 10× monthly is a proposed default, not a decision.
- No visual regression tests; layout was reviewed manually via screenshots (landing light, dashboard dark, mobile).
- Legal pages need counsel review.

