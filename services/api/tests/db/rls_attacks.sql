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
select pg_temp.as_ctx(:'alice', :'org_a');
insert into sessions (user_id, token_hash, expires_at) values (:'alice', decode(repeat('11',32),'hex'), now() + interval '1 day');
select pg_temp.blocked('cannot create a session for another user', format($q$insert into sessions (user_id, token_hash, expires_at) values (%L, decode(repeat('22',32),'hex'), now()+interval '1 day')$q$, :'bob'));
select pg_temp.as_ctx(:'bob', :'org_b');
select pg_temp.ok('B cannot see A''s sessions', (select count(*) from sessions) = 0);
select pg_temp.ok('session resolves by token hash', (select count(*) from app.resolve_session(decode(repeat('11',32),'hex'))) = 1);
select pg_temp.ok('unknown token resolves to nothing', (select count(*) from app.resolve_session(decode(repeat('99',32),'hex'))) = 0);

\echo ALL RLS ATTACK TESTS PASSED
