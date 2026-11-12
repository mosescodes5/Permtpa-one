-- PERMTPA ONE — 0003 SECURITY DEFINER helpers. They run as the (non-runtime) migrator/owner role, which bypasses RLS
-- by being the table owner. Each pins search_path and is EXECUTE-restricted to permtpa_app.

-- The active organization for this transaction — but ONLY if the current user is an active member of it.
-- Even if the server mistakenly sets app.org_id to another tenant, this returns NULL and RLS denies access.
create function app.current_org_id() returns uuid language sql stable security definer set search_path = public, pg_temp as $$
  select m.org_id from memberships m
  where m.org_id = nullif(current_setting('app.org_id', true), '')::uuid
    and m.user_id = app.current_user_id() and m.status = 'active' $$;

create function app.current_role() returns org_role language sql stable security definer set search_path = public, pg_temp as $$
  select m.role from memberships m
  where m.org_id = nullif(current_setting('app.org_id', true), '')::uuid
    and m.user_id = app.current_user_id() and m.status = 'active' $$;

create function app.is_org_admin() returns boolean language sql stable as $$
  select coalesce(app.current_role() in ('owner','administrator'), false) $$;

-- Registration: the app role has no INSERT on users, so rows can only be created through this audited path.
create function app.register_user(p_email citext, p_full_name text, p_password_hash text) returns uuid
language plpgsql security definer set search_path = public, pg_temp as $$
declare v_id uuid;
begin
  insert into users (email, full_name, password_hash) values (p_email, p_full_name, p_password_hash) returning id into v_id;
  insert into audit_logs (actor_user_id, action, target_type, target_id) values (v_id, 'auth.register', 'user', v_id::text);
  return v_id;
end $$;

-- Pre-authentication lookup (no user context exists yet). Returns only what login needs.
create function app.find_user_for_login(p_email citext)
returns table (id uuid, password_hash text, status text, email_verified_at timestamptz, failed_login_count integer, locked_until timestamptz, mfa_enabled boolean)
language sql stable security definer set search_path = public, pg_temp as $$
  select u.id, u.password_hash, u.status, u.email_verified_at, u.failed_login_count, u.locked_until, u.mfa_enabled from users u where u.email = p_email $$;

-- Session resolution by token hash (cookie -> user).
create function app.resolve_session(p_token_hash bytea) returns table (session_id uuid, user_id uuid, expires_at timestamptz)
language sql stable security definer set search_path = public, pg_temp as $$
  select s.id, s.user_id, s.expires_at from sessions s join users u on u.id = s.user_id
  where s.token_hash = p_token_hash and s.revoked_at is null and s.expires_at > now() and u.status = 'active' $$;

-- Creating an organization atomically makes the caller its Owner. There is no other path to become an owner.
create function app.create_organization(p_name text, p_slug citext, p_business_type text, p_currency char(3) default 'NGN') returns uuid
language plpgsql security definer set search_path = public, pg_temp as $$
declare v_user uuid := app.current_user_id(); v_org uuid;
begin
  if v_user is null then raise exception 'authentication required' using errcode = '42501'; end if;
  insert into organizations (name, slug, business_type, currency, created_by) values (p_name, p_slug, p_business_type, p_currency, v_user) returning id into v_org;
  insert into memberships (org_id, user_id, role) values (v_org, v_user, 'owner');
  insert into audit_logs (org_id, actor_user_id, action, target_type, target_id) values (v_org, v_user, 'org.create', 'organization', v_org::text);
  return v_org;
end $$;

-- Authentication events that happen before a user/org context exists.
create function app.record_auth_event(p_action text, p_user uuid, p_ip inet, p_metadata jsonb default '{}') returns void
language sql security definer set search_path = public, pg_temp as $$
  insert into audit_logs (actor_user_id, action, target_type, target_id, ip, metadata)
  values (p_user, p_action, 'user', p_user::text, p_ip, coalesce(p_metadata, '{}')) $$;

revoke all on function app.current_org_id(), app.current_role(), app.is_org_admin(), app.register_user(citext,text,text), app.find_user_for_login(citext),
  app.resolve_session(bytea), app.create_organization(text,citext,text,char), app.record_auth_event(text,uuid,inet,jsonb) from public;
grant execute on function app.current_org_id(), app.current_role(), app.is_org_admin(), app.register_user(citext,text,text), app.find_user_for_login(citext),
  app.resolve_session(bytea), app.create_organization(text,citext,text,char), app.record_auth_event(text,uuid,inet,jsonb) to permtpa_app;
