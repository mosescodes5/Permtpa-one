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

## Database roles (least privilege)
- `permtpa_migrator` — owns the schema; used **only** for migrations.
- `permtpa_app` — runtime role. Not owner, `NOBYPASSRLS`, column-level grants (no access to `users.password_hash`), no UPDATE/DELETE on `audit_logs`.
Re-running `infrastructure/docker/postgres-init.sh` with new passwords rotates both credentials.

## Tests
| Command | What |
|---|---|
| `npm run lint / typecheck / test` | ESLint, tsc strict, Vitest |
| `npm run test:e2e` | Playwright (desktop + mobile). Set `PW_CHROMIUM_PATH` if browsers cannot be downloaded |
| `cargo test --workspace` | Rust unit + HTTP tests |
| `TEST_ADMIN_DATABASE_URL=postgres://postgres:pw@127.0.0.1:5432/postgres cargo test -p permtpa-api -- --ignored --test-threads=1` | Tenant isolation through the Rust code path |
| `PGHOST=127.0.0.1 PGPASSWORD=... ./scripts/test-db.sh` | 42 adversarial SQL attacks as the runtime role |
| `MUTATE_SQL="…" ./scripts/test-db.sh` | Mutation check: sabotage a policy and confirm the suite fails |
