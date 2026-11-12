# PERMTPA ONE

**One business. One platform. Total control.**
PERMTPA ONE is a business operating system — a technology product by PERMTPA.

Status: **Milestone 1 complete** (see [PROGRESS.md](PROGRESS.md)). Not yet production-ready: authentication, products, sales, billing etc. arrive in Milestones 2–8.

## Layout
| Path | Purpose |
|---|---|
| `apps/web` | Next.js 16 + React 19 frontend (marketing site, auth UI, app shell) |
| `services/api` | Rust/Axum API + PostgreSQL migrations |
| `crates/domain` | Pure domain types (roles, money) — no I/O |
| `packages/design-system` | Design tokens (CSS variables, light/dark) |
| `infrastructure/docker` | Postgres role/database bootstrap |
| `docs/` | Architecture, security, development guides |

## Quick start
See [docs/development.md](docs/development.md). In short: `cp .env.example .env`, edit placeholders, `docker compose up -d`, `cargo run -p permtpa-api -- migrate`, `cargo run -p permtpa-api`, `npm ci && npm run dev`.

## Verify
`npm run lint && npm run typecheck && npm test` · `npm run test:e2e` · `cargo test --workspace` · `./scripts/test-db.sh`
