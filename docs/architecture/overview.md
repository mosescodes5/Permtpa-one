# Architecture overview (Milestone 1)

## Shape
Modular monolith. Next.js (UI) → Rust/Axum API (BFF-style, cookie sessions) → PostgreSQL (+ Redis for rate limits/cache). The API is independent of the frontend; contracts will be OpenAPI-generated (`utoipa`) into `packages/api-client` with a CI drift check.

## Tenancy model (implemented and tested)
1. The API resolves `(user_id, org_id)` **server-side** and calls `begin_tenant_tx`, which sets `app.user_id` / `app.org_id` with `set_config(..., is_local => true)` — discarded at commit/rollback, so it cannot leak across pooled connections (tested with a 1-connection pool).
2. `app.current_org_id()` returns the org only if the user is an **active member** — a forged or mistaken org id yields NULL, so RLS denies by default.
3. RLS is enabled on every tenant table; the runtime role is not the owner, so it always applies. Owner-only SECURITY DEFINER functions are the sole path for registration, org creation, login lookup and session resolution.
4. Role changes are enforced in the database (only owner/admin manage members; only an owner touches owners; last owner is protected). `audit_logs` is append-only via privileges **and** triggers.

## Entity model (M1 tables; later milestones add `org_id` + RLS the same way)
```
users 1─* sessions
users *─* organizations   (via memberships: role, status)
organizations 1─* invitations (role ≠ owner)
organizations 1─* audit_logs (org_id nullable for pre-tenant auth events)
business_types 1─* organizations   (configurable industries, not hard-coded)
```
Planned: products/variants/stock_movements (M3), sales/customers/suppliers (M4), ledger/expenses (M5), subscriptions/billing_payments/webhook_events (M6). Money is `bigint` minor units everywhere.

## Threat model (summary — expands each milestone)
| Threat | Mitigation | Status |
|---|---|---|
| Cross-tenant read/write, IDOR | Server-resolved context + membership-checked org + RLS | Implemented, 42 SQL + 3 Rust tests |
| Privilege escalation | DB-enforced role rules, last-owner trigger | Implemented, tested |
| Password theft | Argon2id PHC hashes; column hidden from runtime role | Column control done; hashing in M2 |
| Context bleed via connection pool | Transaction-local settings | Implemented, tested |
| Audit tampering | No UPDATE/DELETE grant + triggers | Implemented, tested |
| Secret leakage in logs/errors | Redacted `Debug`, generic readiness errors | Implemented, tested |
| Session theft, CSRF, brute force | HttpOnly cookies, rotation, rate limits | **Planned M2** |
| Webhook forgery/replay | Signature + server-side verify + idempotency | **Planned M6** |
| Prompt injection (AI) | Fixed, permission-scoped tools; no free SQL | **Planned M7** |
Residual risks to date: no authentication layer yet; superuser compromise bypasses RLS by design; CSP for the web app is not yet enforced beyond baseline headers.

## Design system
Tokens in `packages/design-system/tokens.css`: midnight navy `#0B1426`, warm white `#FAF8F4`, a single emerald accent (`#0F8A63` light / `#2FC58F` dark). Instrument Serif for display, Inter for UI, JetBrains Mono for data; tabular numerals for money. Light/dark via `data-theme`; motion respects `prefers-reduced-motion`. Fonts are self-hosted (no third-party requests).

## API contract strategy
REST `/v1`, OpenAPI from Rust, generated TS client, problem+json errors, cursor pagination, idempotency keys on money-moving writes.
