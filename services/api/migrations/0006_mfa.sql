-- PERMTPA ONE — 0006 optional multi-factor authentication (TOTP) + single-use recovery codes.
create table user_mfa (
  user_id         uuid primary key references users(id) on delete cascade,
  secret_enc      bytea not null,                 -- AES-256-GCM ciphertext (key lives in the API environment, not the DB)
  confirmed_at    timestamptz,
  last_used_step  bigint not null default 0,      -- replay protection: a TOTP step can be used once
  created_at      timestamptz not null default now()
);
create table mfa_recovery_codes (
  id         uuid primary key default gen_random_uuid(),
  user_id    uuid not null references users(id) on delete cascade,
  code_hash  bytea not null check (octet_length(code_hash) = 32),
  used_at    timestamptz,
  created_at timestamptz not null default now(),
  unique (user_id, code_hash)
);
alter table user_mfa enable row level security;
alter table mfa_recovery_codes enable row level security;
create policy mfa_own on user_mfa for all to permtpa_app using (user_id = app.current_user_id()) with check (user_id = app.current_user_id());
create policy mfa_codes_own on mfa_recovery_codes for all to permtpa_app using (user_id = app.current_user_id()) with check (user_id = app.current_user_id());
grant select, insert, delete on user_mfa, mfa_recovery_codes to permtpa_app;
grant update (confirmed_at, last_used_step) on user_mfa to permtpa_app;
grant update (used_at) on mfa_recovery_codes to permtpa_app;

-- users.mfa_enabled can only be flipped through this function, for the authenticated user themself.
create function app.set_mfa_enabled(p_enabled boolean) returns void
language plpgsql security definer set search_path = public, pg_temp as $$
declare v uuid := app.current_user_id();
begin
  if v is null then raise exception 'authentication required' using errcode = '42501'; end if;
  update users set mfa_enabled = p_enabled where id = v;
  insert into audit_logs (actor_user_id, action, target_type, target_id) values (v, case when p_enabled then 'auth.mfa_enabled' else 'auth.mfa_disabled' end, 'user', v::text);
end $$;
revoke all on function app.set_mfa_enabled(boolean) from public;
grant execute on function app.set_mfa_enabled(boolean) to permtpa_app;
