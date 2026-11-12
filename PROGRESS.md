# PROGRESS LOG — PERMTPA ONE

_Every result below was produced by actually running the command in the build sandbox (Ubuntu 24.04). Nothing here was run in Docker — see Known issues._

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

### Next: Milestone 2 (not started)
Rust auth (Argon2id, cookie sessions + rotation, CSRF, rate limiting, email verification/reset, MFA + recovery codes, session list/revocation), permission matrix, wire the existing forms and shell to the API, extend the attack suite with auth-abuse tests.
