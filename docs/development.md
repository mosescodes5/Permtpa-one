# Development setup

Prerequisites: Node 22+, Rust stable (1.85+; developed on 1.97), Docker (for Postgres/Redis) or local PostgreSQL 16+/Redis 7+.

```bash
cp .env.example .env            # replace every change-me-* value
docker compose --env-file .env up -d
set -a; source .env; set +a
cargo run -p permtpa-api -- migrate     # applies migrations as permtpa_migrator
cargo run -p permtpa-api                # API on API_BIND (default 127.0.0.1:8080)
npm ci && npm run dev                   # web on :3000
```

## Auth configuration
Required: `CSRF_SECRET` (≥32 chars) and `MFA_ENCRYPTION_KEY` (base64, 32 bytes) — generate with `openssl rand -base64 48` / `openssl rand -base64 32`. The API refuses to start without them.
The browser talks only to Next (`/api/*` is proxied to the API via `API_INTERNAL_URL`), so cookies are first-party. Set `TRUSTED_PROXY_HOPS=1` when the API sits behind that proxy.
In development, emails are captured in memory and readable at `GET /v1/dev/outbox` (that route does not exist in staging/production). Set `SMTP_URL` to deliver for real (TLS required outside development).

## Database roles (least privilege)
- `permtpa_migrator` — owns the schema; used **only** for migrations.
- `permtpa_app` — runtime role. Not owner, `NOBYPASSRLS`, column-level grants (no access to `users.password_hash`), no UPDATE/DELETE on `audit_logs`.
Re-running `infrastructure/docker/postgres-init.sh` with new passwords rotates both credentials.

## Tests
| Command | What |
|---|---|
| `npm run lint / typecheck / test` | ESLint, tsc strict, Vitest |
| `./scripts/e2e-setup.sh && npm run test:e2e` | Full-stack Playwright (Next + Rust API + Postgres + Redis; desktop + mobile). Build the API first (`cargo build -p permtpa-api`). Set `PW_CHROMIUM_PATH` if browsers cannot be downloaded |
| `cargo test --workspace` | Rust unit + HTTP tests |
| `TEST_ADMIN_DATABASE_URL=postgres://postgres:pw@127.0.0.1:5432/postgres cargo test -p permtpa-api -- --ignored --test-threads=1` | Tenant isolation through the Rust code path |
| `TEST_ADMIN_DATABASE_URL=… cargo test -p permtpa-api -- --ignored` | HTTP auth/CSRF/MFA/tenancy suites (needs Postgres + Redis; SMTP tests need `scripts/smtp-sink.py` and `TEST_SMTP_URL`/`TEST_SMTP_LOG`) |
| `PGHOST=127.0.0.1 PGPASSWORD=... ./scripts/test-db.sh` | 67 adversarial SQL attacks as the runtime role |
| `MUTATE_SQL="…" ./scripts/test-db.sh` | Mutation check: sabotage a policy and confirm the suite fails |
