-- Adversarial tenant-isolation / privilege-escalation tests. Run AS the runtime role (permtpa_app).
-- Any failed assertion raises an exception and aborts the run (ON_ERROR_STOP).
\set ON_ERROR_STOP on
\set QUIET on
set client_min_messages = notice;

create function pg_temp.ok(name text, cond boolean) returns void language plpgsql as $$
begin if coalesce(cond,false) then raise notice 'PASS  %', name; else raise exception 'FAIL  %', name; end if; end $$;

-- Expects the statement to be rejected by RLS (42501), a CHECK (23514), a trigger (P0001) or a privilege error.
create function pg_temp.blocked(name text, stmt text) returns void language plpgsql as $$
declare allowed boolean := false;
begin
  begin
    execute stmt;
    allowed := true;   -- only reached if NOTHING rejected the statement
  exception when sqlstate '42501' or sqlstate '23514' or sqlstate 'P0001' then
    null;              -- rejected by RLS / privilege / CHECK / trigger: expected
  end;
  -- Raised OUTSIDE the protected block so the handler above can never swallow it.
  if allowed then raise exception 'FAIL  % (statement was ALLOWED)', name; end if;
  raise notice 'PASS  % (blocked)', name;
end $$;

-- Returns how many rows a DML statement touched (0 => RLS silently hid them).
create function pg_temp.affected(stmt text) returns integer language plpgsql as $$
declare n integer; begin execute stmt; get diagnostics n = row_count; return n; end $$;

create function pg_temp.as_ctx(u uuid, o uuid) returns void language sql as $$
  select set_config('app.user_id', coalesce(u::text,''), false), set_config('app.org_id', coalesce(o::text,''), false) $$;

-- ---------- fixtures (through the only sanctioned paths) ----------
select app.register_user('alice@a.test','Alice Owner','$argon2id$stub') as alice \gset
select app.register_user('bob@b.test','Bob Owner','$argon2id$stub') as bob \gset
select app.register_user('carol@a.test','Carol Sales','$argon2id$stub') as carol \gset
select app.register_user('dave@a.test','Dave Admin','$argon2id$stub') as dave \gset

select pg_temp.as_ctx(:'alice', null);
select app.create_organization('Org A','org-a','retail') as org_a \gset
select pg_temp.as_ctx(:'bob', null);
select app.create_organization('Org B','org-b','agriculture') as org_b \gset

-- Alice (owner of A) adds Carol (sales_staff) and Dave (administrator)
select pg_temp.as_ctx(:'alice', :'org_a');
insert into memberships (org_id, user_id, role) values (:'org_a', :'carol', 'sales_staff'), (:'org_a', :'dave', 'administrator');
select pg_temp.ok('owner can add members', (select count(*) from memberships where org_id = :'org_a') = 3);

-- ---------- 1. tenant isolation ----------
select pg_temp.ok('A sees only org A', (select count(*) from organizations) = 1 and (select id from organizations) = :'org_a');
select pg_temp.as_ctx(:'bob', :'org_b');
select pg_temp.ok('B sees only org B', (select count(*) from organizations) = 1 and (select id from organizations) = :'org_b');
select pg_temp.ok('B cannot see org A memberships', (select count(*) from memberships where org_id = :'org_a') = 0);

-- 2. server bug / forged header: Bob sets org context to A
select pg_temp.as_ctx(:'bob', :'org_a');
select pg_temp.ok('forged org context: no org A memberships visible', (select count(*) from memberships where org_id = :'org_a') = 0);
select pg_temp.ok('forged org context: no org A row visible', (select count(*) from organizations where id = :'org_a') = 0);
select pg_temp.blocked('forged org context: cannot write audit into org A', format($q$insert into audit_logs (org_id, actor_user_id, action) values (%L, %L, 'evil.write')$q$, :'org_a', :'bob'));
select pg_temp.blocked('forged org context: cannot invite into org A', format($q$insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values (%L,'x@y.z','manager', decode(repeat('ab',32),'hex'), %L, now()+interval '1 day')$q$, :'org_a', :'bob'));
select pg_temp.blocked('forged org context: cannot add self to org A', format($q$insert into memberships (org_id, user_id, role) values (%L, %L, 'administrator')$q$, :'org_a', :'bob'));

-- 3. no context at all => deny by default
select pg_temp.as_ctx(null, null);
select pg_temp.ok('no context: zero orgs', (select count(*) from organizations) = 0);
select pg_temp.ok('no context: zero memberships', (select count(*) from memberships) = 0);
select pg_temp.ok('no context: zero users', (select count(*) from users) = 0);
select pg_temp.blocked('no context: cannot create organization', $q$select app.create_organization('Ghost Org','ghost-org','retail')$q$);

-- ---------- 4. privilege escalation ----------
select pg_temp.as_ctx(:'carol', :'org_a');
select pg_temp.ok('sales_staff cannot self-promote (0 rows)', pg_temp.affected(format($q$update memberships set role='owner' where user_id=%L$q$, :'carol')) = 0);
select pg_temp.ok('sales_staff cannot self-promote to admin (0 rows)', pg_temp.affected(format($q$update memberships set role='administrator' where user_id=%L$q$, :'carol')) = 0);
select pg_temp.blocked('sales_staff cannot add members', format($q$insert into memberships (org_id, user_id, role) values (%L, %L, 'read_only')$q$, :'org_a', :'bob'));
select pg_temp.ok('sales_staff cannot delete members (0 rows)', pg_temp.affected(format($q$delete from memberships where org_id=%L and user_id<>%L$q$, :'org_a', :'carol')) = 0);
select pg_temp.ok('sales_staff cannot read audit logs', (select count(*) from audit_logs) = 0);
select pg_temp.ok('sales_staff cannot read invitations', (select count(*) from invitations) = 0);

select pg_temp.as_ctx(:'dave', :'org_a');
select pg_temp.blocked('admin cannot grant owner', format($q$update memberships set role='owner' where user_id=%L$q$, :'dave'));
select pg_temp.ok('admin cannot modify the owner row (0 rows)', pg_temp.affected(format($q$update memberships set role='read_only' where user_id=%L$q$, :'alice')) = 0);
select pg_temp.ok('admin cannot delete the owner (0 rows)', pg_temp.affected(format($q$delete from memberships where user_id=%L$q$, :'alice')) = 0);
select pg_temp.blocked('admin cannot insert an owner', format($q$insert into memberships (org_id, user_id, role) values (%L, %L, 'owner')$q$, :'org_a', :'bob'));
select pg_temp.ok('admin CAN manage lower roles', pg_temp.affected(format($q$update memberships set role='manager' where user_id=%L$q$, :'carol')) = 1);
select pg_temp.blocked('invitation for owner role rejected by CHECK', format($q$insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values (%L,'o@x.z','owner', decode(repeat('cd',32),'hex'), %L, now()+interval '1 day')$q$, :'org_a', :'dave'));
select pg_temp.blocked('cannot forge invited_by', format($q$insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values (%L,'f@x.z','manager', decode(repeat('ef',32),'hex'), %L, now()+interval '1 day')$q$, :'org_a', :'alice'));

-- last-owner protection
select pg_temp.as_ctx(:'alice', :'org_a');
select pg_temp.blocked('sole owner cannot demote self', format($q$update memberships set role='administrator' where user_id=%L$q$, :'alice'));
select pg_temp.blocked('sole owner cannot be removed', format($q$delete from memberships where user_id=%L$q$, :'alice'));

-- ---------- 5. audit log immutability ----------
select pg_temp.ok('owner can read audit log', (select count(*) from audit_logs where org_id = :'org_a') >= 1);
select pg_temp.blocked('app role cannot UPDATE audit_logs', $q$update audit_logs set action='tampered'$q$);
select pg_temp.blocked('app role cannot DELETE audit_logs', $q$delete from audit_logs$q$);
select pg_temp.blocked('app role cannot TRUNCATE audit_logs', $q$truncate audit_logs$q$);
select pg_temp.blocked('cannot spoof actor in audit entry', format($q$insert into audit_logs (org_id, actor_user_id, action) values (%L, %L, 'spoof')$q$, :'org_a', :'bob'));

-- ---------- 6. secrets & users ----------
select pg_temp.blocked('password_hash is not selectable', $q$select password_hash from users$q$);
select pg_temp.blocked('cannot insert users directly', $q$insert into users (email, full_name, password_hash) values ('evil@x.z','Evil','x')$q$);
select pg_temp.blocked('cannot mark own email verified', format($q$update users set email_verified_at = now() where id = %L$q$, :'alice'));
select pg_temp.ok('cannot update another user (0 rows)', pg_temp.affected(format($q$update users set full_name='Hacked Name' where id = %L$q$, :'bob')) = 0);
select pg_temp.ok('user list is limited to self + active-org members', (select count(*) from users) = 3);

-- ---------- 7. sessions ----------
select pg_temp.as_ctx(null, null);
select app.issue_email_token(:'alice', 'verify_email', decode(repeat('5a',32),'hex'), interval '1 hour');
select app.verify_email(decode(repeat('5a',32),'hex'));
select pg_temp.as_ctx(:'alice', :'org_a');
insert into sessions (user_id, token_hash, expires_at) values (:'alice', decode(repeat('11',32),'hex'), now() + interval '1 day');
select pg_temp.blocked('cannot create a session for another user', format($q$insert into sessions (user_id, token_hash, expires_at) values (%L, decode(repeat('22',32),'hex'), now()+interval '1 day')$q$, :'bob'));
select pg_temp.as_ctx(:'bob', :'org_b');
select pg_temp.ok('B cannot see A''s sessions', (select count(*) from sessions) = 0);
select pg_temp.ok('session resolves by token hash', (select count(*) from app.resolve_session(decode(repeat('11',32),'hex'))) = 1);
select pg_temp.ok('unknown token resolves to nothing', (select count(*) from app.resolve_session(decode(repeat('99',32),'hex'))) = 0);


-- ================= Milestone 2: tokens, lockout, MFA =================
select pg_temp.as_ctx(null, null);
select pg_temp.blocked('email_tokens is not directly readable', $q$select * from email_tokens$q$);
select pg_temp.blocked('email_tokens cannot be written directly', $q$insert into email_tokens (user_id, purpose, token_hash, expires_at) values (gen_random_uuid(),'verify_email',decode(repeat('aa',32),'hex'),now())$q$);
select pg_temp.blocked('consume_email_token is not callable by the app role', $q$select app.consume_email_token(decode(repeat('aa',32),'hex'),'reset_password')$q$);

select app.register_user('erin@m2.test','Erin Token','$argon2id$old') as erin \gset
select app.issue_email_token(:'erin', 'verify_email', decode(repeat('a1',32),'hex'), interval '1 hour');
-- a session row that exists for a still-unverified user must be unusable
select pg_temp.as_ctx(:'erin', null);
insert into sessions (user_id, token_hash, expires_at) values (:'erin', decode(repeat('81',32),'hex'), now() + interval '1 day');
select pg_temp.as_ctx(null, null);
select pg_temp.ok('unverified user cannot resolve a session', (select count(*) from app.resolve_session(decode(repeat('81',32),'hex'))) = 0);
select pg_temp.ok('verify with wrong token fails', app.verify_email(decode(repeat('b2',32),'hex')) is null);
select pg_temp.ok('verify with right token succeeds', app.verify_email(decode(repeat('a1',32),'hex')) = :'erin'::uuid);
select pg_temp.ok('the same session works once the email is verified', (select count(*) from app.resolve_session(decode(repeat('81',32),'hex'))) = 1);
select pg_temp.ok('verification token is single-use', app.verify_email(decode(repeat('a1',32),'hex')) is null);

-- expiry
select app.issue_email_token(:'erin', 'reset_password', decode(repeat('c3',32),'hex'), interval '-1 minute');
select pg_temp.ok('expired reset token is rejected', app.reset_password(decode(repeat('c3',32),'hex'), '$argon2id$new') is null);
-- purpose confusion: a verify token cannot reset a password
select app.issue_email_token(:'erin', 'verify_email', decode(repeat('d4',32),'hex'), interval '1 hour');
select pg_temp.ok('verify token cannot be used as reset token', app.reset_password(decode(repeat('d4',32),'hex'), '$argon2id$evil') is null);
-- re-issuing invalidates the previous link
select app.issue_email_token(:'erin', 'reset_password', decode(repeat('e5',32),'hex'), interval '1 hour');
select app.issue_email_token(:'erin', 'reset_password', decode(repeat('f6',32),'hex'), interval '1 hour');
select pg_temp.ok('older reset link is invalidated by a newer one', app.reset_password(decode(repeat('e5',32),'hex'), '$argon2id$evil') is null);

-- reset revokes every session
select pg_temp.as_ctx(:'erin', null);
insert into sessions (user_id, token_hash, expires_at) values (:'erin', decode(repeat('71',32),'hex'), now() + interval '1 day');
select pg_temp.ok('session resolves before reset', (select count(*) from app.resolve_session(decode(repeat('71',32),'hex'))) = 1);
select pg_temp.as_ctx(null, null);
select pg_temp.ok('reset with valid token succeeds', app.reset_password(decode(repeat('f6',32),'hex'), '$argon2id$new') = :'erin'::uuid);
select pg_temp.ok('password reset revoked all sessions', (select count(*) from app.resolve_session(decode(repeat('71',32),'hex'))) = 0);
select pg_temp.ok('reset link is single-use', app.reset_password(decode(repeat('f6',32),'hex'), '$argon2id$again') is null);

-- progressive lockout
select pg_temp.ok('4 failures do not lock', (select count(*) from (select app.record_login_failure(:'erin', null) from generate_series(1,4)) x) = 4 and (select locked_until from app.find_user_for_login('erin@m2.test')) is null);
select pg_temp.ok('5th failure locks the account', app.record_login_failure(:'erin', null) > now());
select pg_temp.ok('lock grows with further failures', app.record_login_failure(:'erin', null) > now() + interval '90 seconds');
select app.record_login_success(:'erin', null);
select pg_temp.ok('success clears counter and lock', (select failed_login_count = 0 and locked_until is null from app.find_user_for_login('erin@m2.test')));

-- MFA isolation
select app.register_user('frank@m2.test','Frank Other','$argon2id$x') as frank \gset
select pg_temp.as_ctx(:'erin', null);
insert into user_mfa (user_id, secret_enc) values (:'erin', '\x0102'::bytea);
insert into mfa_recovery_codes (user_id, code_hash) values (:'erin', decode(repeat('99',32),'hex'));
select pg_temp.as_ctx(:'frank', null);
select pg_temp.ok('other users cannot read MFA secrets', (select count(*) from user_mfa) = 0);
select pg_temp.ok('other users cannot read recovery codes', (select count(*) from mfa_recovery_codes) = 0);
select pg_temp.blocked('cannot plant MFA secret for another user', format($q$insert into user_mfa (user_id, secret_enc) values (%L, '\x01'::bytea)$q$, :'erin'));
select pg_temp.ok('cannot burn another user''s recovery code (0 rows)', pg_temp.affected(format($q$update mfa_recovery_codes set used_at = now() where user_id = %L$q$, :'erin')) = 0);
select pg_temp.blocked('cannot flip mfa_enabled directly', format($q$update users set mfa_enabled = false where id = %L$q$, :'frank'));
select pg_temp.as_ctx(null, null);
select pg_temp.blocked('set_mfa_enabled requires authentication', $q$select app.set_mfa_enabled(true)$q$);

\echo ALL RLS ATTACK TESTS PASSED
