# PROGRESS LOG — PERMTPA ONE

_Every result below was produced by actually running the command in the build sandbox (Ubuntu 24.04). Nothing here was run in Docker — see Known issues._

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
- `TRUSTED_PROXY_HOPS` must match deployment topology; e2e deliberately uses 2 so tests can present unique client IPs.
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

