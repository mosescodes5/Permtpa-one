#!/usr/bin/env bash
# Creates least-privilege roles and the dev database. Runs once via the postgres image's /docker-entrypoint-initdb.d,
# or manually:  APP_DB_PASSWORD=... MIGRATOR_DB_PASSWORD=... ./postgres-init.sh
set -euo pipefail
: "${APP_DB_PASSWORD:?APP_DB_PASSWORD is required}" "${MIGRATOR_DB_PASSWORD:?MIGRATOR_DB_PASSWORD is required}"
DB_NAME="${APP_DB_NAME:-${POSTGRES_DB:-permtpa_one}}"
PSQL=(psql -v ON_ERROR_STOP=1 --username "${POSTGRES_USER:-postgres}" --dbname "${POSTGRES_INIT_DB:-postgres}")
"${PSQL[@]}" <<SQL
do \$\$ begin
  if not exists (select from pg_roles where rolname = 'permtpa_migrator') then
    create role permtpa_migrator login password '${MIGRATOR_DB_PASSWORD}' nosuperuser nocreaterole nobypassrls;
  end if;
  if not exists (select from pg_roles where rolname = 'permtpa_app') then
    create role permtpa_app login password '${APP_DB_PASSWORD}' nosuperuser nocreatedb nocreaterole nobypassrls;
  end if;
end \$\$;
-- Idempotent password (re)set: re-running this script rotates credentials.
alter role permtpa_migrator password '${MIGRATOR_DB_PASSWORD}';
alter role permtpa_app password '${APP_DB_PASSWORD}';
SQL
"${PSQL[@]}" -tc "select 1 from pg_database where datname='${DB_NAME}'" | grep -q 1 || "${PSQL[@]}" -c "create database ${DB_NAME} owner permtpa_migrator"
"${PSQL[@]}" -c "revoke all on database ${DB_NAME} from public; grant connect on database ${DB_NAME} to permtpa_app;"
