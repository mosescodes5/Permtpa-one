-- PERMTPA ONE — 0005 authentication support: single-use email tokens, lockout accounting, safe session resolution.
-- email_tokens is reachable ONLY through the SECURITY DEFINER functions below (RLS on, no policy, no grant).

create table email_tokens (
  id         uuid primary key default gen_random_uuid(),
  user_id    uuid not null references users(id) on delete cascade,
  purpose    text not null check (purpose in ('verify_email','reset_password')),
  token_hash bytea not null unique check (octet_length(token_hash) = 32),   -- SHA-256; raw token only ever lives in the email
  expires_at timestamptz not null,
  used_at    timestamptz,
  created_at timestamptz not null default now()
);
create index email_tokens_user_idx on email_tokens (user_id, purpose) where used_at is null;
alter table email_tokens enable row level security;

create function app.issue_email_token(p_user uuid, p_purpose text, p_token_hash bytea, p_ttl interval) returns void
language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if p_purpose not in ('verify_email','reset_password') then raise exception 'invalid token purpose'; end if;
  -- Issuing a new token invalidates earlier unused links of the same purpose.
  update email_tokens set used_at = now() where user_id = p_user and purpose = p_purpose and used_at is null;
  insert into email_tokens (user_id, purpose, token_hash, expires_at) values (p_user, p_purpose, p_token_hash, now() + p_ttl);
end $$;

-- Atomic single-use consumption: concurrent redemptions cannot both succeed.
create function app.consume_email_token(p_token_hash bytea, p_purpose text) returns uuid
language sql security definer set search_path = public, pg_temp as $$
  update email_tokens set used_at = now()
  where token_hash = p_token_hash and purpose = p_purpose and used_at is null and expires_at > now()
  returning user_id $$;

create function app.verify_email(p_token_hash bytea) returns uuid
language plpgsql security definer set search_path = public, pg_temp as $$
declare v uuid;
begin
  v := app.consume_email_token(p_token_hash, 'verify_email');
  if v is null then return null; end if;
  update users set email_verified_at = coalesce(email_verified_at, now()) where id = v;
  insert into audit_logs (actor_user_id, action, target_type, target_id) values (v, 'auth.email_verified', 'user', v::text);
  return v;
end $$;

-- Reset: new hash, clear lockout, revoke EVERY session (a stolen session must not survive a reset).
create function app.reset_password(p_token_hash bytea, p_new_password_hash text) returns uuid
language plpgsql security definer set search_path = public, pg_temp as $$
declare v uuid;
begin
  v := app.consume_email_token(p_token_hash, 'reset_password');
  if v is null then return null; end if;
  update users set password_hash = p_new_password_hash, failed_login_count = 0, locked_until = null,
                   email_verified_at = coalesce(email_verified_at, now()) where id = v;
  update sessions set revoked_at = now() where user_id = v and revoked_at is null;
  insert into audit_logs (actor_user_id, action, target_type, target_id) values (v, 'auth.password_reset', 'user', v::text);
  return v;
end $$;

-- Progressive lockout: from the 5th consecutive failure, lock for 1,2,4,… minutes (cap 60).
create function app.record_login_failure(p_user uuid, p_ip inet) returns timestamptz
language plpgsql security definer set search_path = public, pg_temp as $$
declare v timestamptz;
begin
  update users set
    failed_login_count = failed_login_count + 1,
    locked_until = case when failed_login_count + 1 >= 5
      then now() + least(interval '1 minute' * power(2, failed_login_count + 1 - 5), interval '60 minutes') else locked_until end
  where id = p_user returning locked_until into v;
  insert into audit_logs (actor_user_id, action, target_type, target_id, ip) values (p_user, 'auth.login_failed', 'user', p_user::text, p_ip);
  return v;
end $$;

create function app.record_login_success(p_user uuid, p_ip inet) returns void
language plpgsql security definer set search_path = public, pg_temp as $$
begin
  update users set failed_login_count = 0, locked_until = null where id = p_user;
  insert into audit_logs (actor_user_id, action, target_type, target_id, ip) values (p_user, 'auth.login', 'user', p_user::text, p_ip);
end $$;

-- Return last_seen_at so the API can enforce idle timeout. (Return type changed => drop & recreate.)
drop function app.resolve_session(bytea);
create function app.resolve_session(p_token_hash bytea) returns table (session_id uuid, user_id uuid, expires_at timestamptz, last_seen_at timestamptz)
language sql stable security definer set search_path = public, pg_temp as $$
  select s.id, s.user_id, s.expires_at, s.last_seen_at from sessions s join users u on u.id = s.user_id
  where s.token_hash = p_token_hash and s.revoked_at is null and s.expires_at > now() and u.status = 'active' and u.email_verified_at is not null $$;

revoke all on function app.issue_email_token(uuid,text,bytea,interval), app.consume_email_token(bytea,text), app.verify_email(bytea),
  app.reset_password(bytea,text), app.record_login_failure(uuid,inet), app.record_login_success(uuid,inet), app.resolve_session(bytea) from public;
grant execute on function app.issue_email_token(uuid,text,bytea,interval), app.verify_email(bytea), app.reset_password(bytea,text),
  app.record_login_failure(uuid,inet), app.record_login_success(uuid,inet), app.resolve_session(bytea) to permtpa_app;
-- NOTE: consume_email_token is internal; the app role cannot call it directly.
