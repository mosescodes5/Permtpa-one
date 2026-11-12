-- PERMTPA ONE — 0001 foundation: extensions, helper schema, request-context functions.
-- Runtime request context is set PER TRANSACTION by the API:
--   SELECT set_config('app.user_id', $1, true), set_config('app.org_id', $2, true);
create extension if not exists citext;
create extension if not exists pgcrypto;

create schema if not exists app;
revoke all on schema app from public;
grant usage on schema app to permtpa_app;

create type org_role as enum ('owner','administrator','manager','accountant','inventory_manager','sales_staff','read_only');

create function app.set_updated_at() returns trigger language plpgsql as $$
begin new.updated_at = now(); return new; end $$;

-- Authenticated user for this transaction (NULL when unset => RLS denies everything).
create function app.current_user_id() returns uuid language sql stable as $$
  select nullif(current_setting('app.user_id', true), '')::uuid $$;
