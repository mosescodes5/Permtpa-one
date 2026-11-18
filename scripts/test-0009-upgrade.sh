#!/usr/bin/env bash
# Applies 0001-0008, seeds whole-unit data, applies 0009, then verifies the data was rescaled x1000. UNVERIFIED until run.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SU=${PG_SUPERUSER:-postgres}
DB="permtpa_one_upg_$$"
export APP_DB_PASSWORD=${APP_DB_PASSWORD:-test_app_pw} MIGRATOR_DB_PASSWORD=${MIGRATOR_DB_PASSWORD:-test_migrator_pw}
HOST_ARGS=(-h "${PGHOST:-127.0.0.1}" -p "${PGPORT:-5432}")
cleanup() { psql "${HOST_ARGS[@]}" -U "$SU" -d postgres -qc "drop database if exists $DB" >/dev/null 2>&1 || true; }
trap cleanup EXIT
POSTGRES_USER=$SU POSTGRES_DB=$DB bash "$ROOT/infrastructure/docker/postgres-init.sh" >/dev/null
mig() { PGPASSWORD=$MIGRATOR_DB_PASSWORD psql "${HOST_ARGS[@]}" -U permtpa_migrator -d "$DB" -v ON_ERROR_STOP=1 -q -f "$1"; }
for f in "$ROOT"/services/api/migrations/000[1-8]_*.sql; do mig "$f"; done
PGPASSWORD=$APP_DB_PASSWORD psql "${HOST_ARGS[@]}" -U permtpa_app -d "$DB" -q -v ON_ERROR_STOP=1 -f "$ROOT/services/api/tests/db/upgrade_0009_seed.sql"
mig "$ROOT/services/api/migrations/0009_fractional_quantities.sql"
OUT=$(psql "${HOST_ARGS[@]}" -U "$SU" -d "$DB" -q -f "$ROOT/services/api/tests/db/upgrade_0009_verify.sql" 2>&1 || true)
echo "$OUT"
echo "$OUT" | grep -q "PASS 0009 upgrade preserved data" || { echo "UPGRADE TEST FAILED"; exit 1; }
