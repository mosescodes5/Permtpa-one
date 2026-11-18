#!/usr/bin/env bash
# Prepares an isolated database for the full-stack Playwright run and applies migrations with the real migrator role.
# Needs: a PostgreSQL superuser (PGHOST/PGPORT/PGPASSWORD), and `cargo build -p permtpa-api`.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export POSTGRES_USER=${PG_SUPERUSER:-postgres} POSTGRES_DB=permtpa_one_e2e APP_DB_PASSWORD=e2e_app_pw MIGRATOR_DB_PASSWORD=e2e_migrator_pw
# Fresh database every run so tests never depend on leftovers.
psql -h "${PGHOST:-127.0.0.1}" -U "$POSTGRES_USER" -d postgres -qc "drop database if exists permtpa_one_e2e with (force)" 2>/dev/null || true
bash "$ROOT/infrastructure/docker/postgres-init.sh" >/dev/null
export APP_ENV=development DATABASE_URL="postgres://permtpa_app:e2e_app_pw@127.0.0.1:5432/permtpa_one_e2e" \
  MIGRATOR_DATABASE_URL="postgres://permtpa_migrator:e2e_migrator_pw@127.0.0.1:5432/permtpa_one_e2e" REDIS_URL="redis://127.0.0.1:6379" \
  CSRF_SECRET="e2e-only-csrf-secret-0123456789abcdef" MFA_ENCRYPTION_KEY="AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="
"$ROOT/target/debug/permtpa-api" migrate 2>&1 | grep -E "migrations applied|rror" || true
echo "e2e database ready"
