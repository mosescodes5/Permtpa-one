#!/usr/bin/env bash
# Applies migrations to a throwaway database and runs the adversarial RLS suite as the runtime role.
# Requires a reachable PostgreSQL superuser:  PGHOST/PGPORT/PGUSER/PGPASSWORD (defaults: local socket, postgres).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SU=${PG_SUPERUSER:-postgres}
DB="permtpa_one_test_$$"
export APP_DB_PASSWORD=${APP_DB_PASSWORD:-test_app_pw} MIGRATOR_DB_PASSWORD=${MIGRATOR_DB_PASSWORD:-test_migrator_pw}
HOST_ARGS=(); [ -n "${PGHOST:-}" ] && HOST_ARGS=(-h "$PGHOST" -p "${PGPORT:-5432}")
cleanup() { psql "${HOST_ARGS[@]}" -U "$SU" -d postgres -qc "drop database if exists $DB" >/dev/null 2>&1 || true; }
trap cleanup EXIT
POSTGRES_USER=$SU POSTGRES_DB=$DB bash "$ROOT/infrastructure/docker/postgres-init.sh" >/dev/null
for f in "$ROOT"/services/api/migrations/*.sql; do
  PGPASSWORD=$MIGRATOR_DB_PASSWORD psql "${HOST_ARGS[@]:--h 127.0.0.1}" -U permtpa_migrator -d "$DB" -v ON_ERROR_STOP=1 -q -f "$f"
  echo "applied $(basename "$f")"
done
# Test-only: the helpers live in pg_temp. The production role is NOT granted TEMPORARY (see postgres-init.sh).
psql "${HOST_ARGS[@]:--h 127.0.0.1}" -U "$SU" -d postgres -qc "grant temporary on database $DB to permtpa_app"
# Optional: MUTATE_SQL="..." sabotages the schema (as migrator) to prove the suite can actually fail.
[ -n "${MUTATE_SQL:-}" ] && PGPASSWORD=$MIGRATOR_DB_PASSWORD psql "${HOST_ARGS[@]:--h 127.0.0.1}" -U permtpa_migrator -d "$DB" -q -c "$MUTATE_SQL"
OUT=$(PGPASSWORD=$APP_DB_PASSWORD psql "${HOST_ARGS[@]:--h 127.0.0.1}" -U permtpa_app -d "$DB" -q -t -A -f "$ROOT/services/api/tests/db/rls_attacks.sql" 2>&1 || true)
echo "$OUT" | grep -E "NOTICE:  (PASS|FAIL)|ERROR:|ALL RLS" | sed -E 's/^psql:[^:]+:[0-9]+: //; s/NOTICE:  //'
echo "---"; echo "passed: $(echo "$OUT" | grep -c 'NOTICE:  PASS')   failed: $(echo "$OUT" | grep -c 'ERROR:  FAIL')"
echo "$OUT" | grep -q "ALL RLS ATTACK TESTS PASSED" || exit 1
