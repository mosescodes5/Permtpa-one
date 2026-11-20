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
  exception when sqlstate '42501' or sqlstate '23514' or sqlstate '23505' or sqlstate '23503' or sqlstate 'P0001' then
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


-- ================= Milestone 3A: invitations + audit triggers =================
select app.register_user('gina@m3.test','Gina Invitee','$argon2id$x') as gina \gset
select app.register_user('hank@m3.test','Hank Other','$argon2id$x') as hank \gset
select app.issue_email_token(:'gina','verify_email',decode(repeat('c1',32),'hex'),interval '1 hour'); select app.verify_email(decode(repeat('c1',32),'hex'));
select app.issue_email_token(:'hank','verify_email',decode(repeat('c2',32),'hex'),interval '1 hour'); select app.verify_email(decode(repeat('c2',32),'hex'));

select pg_temp.as_ctx(:'alice', :'org_a');
insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values (:'org_a','gina@m3.test','manager', decode(repeat('d1',32),'hex'), :'alice', now() + interval '1 day');
select pg_temp.blocked('only one pending invitation per email', format($q$insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values (%L,'gina@m3.test','read_only', decode(repeat('d2',32),'hex'), %L, now()+interval '1 day')$q$, :'org_a', :'alice'));
select pg_temp.ok('invitation creation is audited with the real actor', (select count(*) from audit_logs where org_id = :'org_a' and action = 'invitation.create' and actor_user_id = :'alice') = 1);

select pg_temp.as_ctx(:'hank', null);
select pg_temp.ok('a different user cannot redeem the invitation', app.accept_invitation(decode(repeat('d1',32),'hex')) is null);
select pg_temp.ok('a bad token redeems nothing', app.accept_invitation(decode(repeat('ff',32),'hex')) is null);
select pg_temp.as_ctx(null, null);
select pg_temp.blocked('accepting requires authentication', $q$select app.accept_invitation(decode(repeat('d1',32),'hex'))$q$);

select pg_temp.as_ctx(:'gina', null);
select pg_temp.ok('the invited user can accept', app.accept_invitation(decode(repeat('d1',32),'hex')) = :'org_a'::uuid);
select pg_temp.ok('accepting grants exactly the invited role', (select role from memberships where org_id = :'org_a' and user_id = :'gina') = 'manager');
select pg_temp.ok('an invitation is single-use', app.accept_invitation(decode(repeat('d1',32),'hex')) is null);
select pg_temp.as_ctx(:'alice', :'org_a');
select pg_temp.ok('acceptance and member.add are audited', (select count(*) from audit_logs where org_id = :'org_a' and action in ('invitation.accepted','member.add') and target_id = :'gina') = 2);

-- expired / revoked invitations
select pg_temp.as_ctx(:'alice', :'org_a');
insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values (:'org_a','hank@m3.test','read_only', decode(repeat('d3',32),'hex'), :'alice', now() - interval '1 minute');
select pg_temp.as_ctx(:'hank', null);
select pg_temp.ok('an expired invitation cannot be accepted', app.accept_invitation(decode(repeat('d3',32),'hex')) is null);
select pg_temp.as_ctx(:'alice', :'org_a');
insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) select :'org_a','hank@m3.test','sales_staff', decode(repeat('d4',32),'hex'), :'alice', now() + interval '1 day' where false;
update invitations set revoked_at = now() where token_hash = decode(repeat('d3',32),'hex');
insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values (:'org_a','hank@m3.test','sales_staff', decode(repeat('d5',32),'hex'), :'alice', now() + interval '1 day');
update invitations set revoked_at = now() where token_hash = decode(repeat('d5',32),'hex');
select pg_temp.as_ctx(:'hank', null);
select pg_temp.ok('a revoked invitation cannot be accepted', app.accept_invitation(decode(repeat('d5',32),'hex')) is null);
select pg_temp.as_ctx(:'alice', :'org_a');
select pg_temp.ok('revocation is audited', (select count(*) from audit_logs where org_id = :'org_a' and action = 'invitation.revoke') >= 1);

-- role changes / removals are audited and hidden from non-admins
select pg_temp.as_ctx(:'alice', :'org_a');
update memberships set role = 'accountant' where user_id = :'gina';
select pg_temp.ok('role change is audited with before/after', (select (metadata->>'from_role') = 'manager' and (metadata->>'to_role') = 'accountant' from audit_logs where org_id = :'org_a' and action = 'member.role_change' and target_id = :'gina'));
delete from memberships where user_id = :'gina';
select pg_temp.ok('member removal is audited', (select count(*) from audit_logs where org_id = :'org_a' and action = 'member.remove' and target_id = :'gina') = 1);
select pg_temp.as_ctx(:'carol', :'org_a');
select pg_temp.ok('non-admins cannot read the audit trail', (select count(*) from audit_logs) = 0);
select pg_temp.blocked('audit rows cannot be forged by members', format($q$insert into audit_logs (org_id, actor_user_id, action) values (%L, %L, 'member.add')$q$, :'org_a', :'alice'));


-- ================= Milestone 3B: inventory =================
select app.register_user('ivy@m3b.test','Ivy Inventory','$argon2id$x') as ivy \gset
select app.register_user('sam@m3b.test','Sam Sales','$argon2id$x') as sam \gset
select app.register_user('rob@m3b.test','Rob Reader','$argon2id$x') as rob \gset
select app.register_user('acc@m3b.test','Acc Accountant','$argon2id$x') as acc \gset
select app.register_user('mia@m3b.test','Mia Manager','$argon2id$x') as mia \gset
select pg_temp.as_ctx(:'alice', :'org_a');
insert into memberships (org_id, user_id, role) values (:'org_a', :'ivy', 'inventory_manager'), (:'org_a', :'sam', 'sales_staff'), (:'org_a', :'rob', 'read_only'), (:'org_a', :'acc', 'accountant'), (:'org_a', :'mia', 'manager');

-- inventory manager builds the catalogue
select pg_temp.as_ctx(:'ivy', :'org_a');
insert into categories (org_id, name) values (:'org_a', 'Groceries') returning id as cat_a \gset
insert into products (org_id, category_id, name, created_by) values (:'org_a', :'cat_a', 'Rice 50kg', :'ivy') returning id as prod_a \gset
insert into variants (org_id, product_id, sku, barcode, price_minor, reorder_level) values (:'org_a', :'prod_a', 'RIC-050', '6001234567890', 1600000, 5000) returning id as var_a \gset
insert into variant_costs (variant_id, org_id, cost_minor) values (:'var_a', :'org_a', 1200000);
insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (:'org_a', :'var_a', 10000, 'opening', :'ivy');
select pg_temp.ok('inventory manager can build a catalogue with opening stock', (select on_hand from stock_levels where variant_id = :'var_a') = 10000);
select pg_temp.ok('movement records the resulting balance', (select resulting_on_hand from stock_movements where variant_id = :'var_a') = 10000);

-- role matrix: sales staff and read-only
select pg_temp.as_ctx(:'sam', :'org_a');
select pg_temp.ok('sales staff can see products, prices and stock', (select count(*) from products) = 1 and (select count(*) from variants) = 1 and (select on_hand from stock_levels) = 10000);
select pg_temp.ok('sales staff CANNOT see cost prices', (select count(*) from variant_costs) = 0);
select pg_temp.blocked('sales staff cannot create products', format($q$insert into products (org_id, name, created_by) values (%L, 'Sneaky', %L)$q$, :'org_a', :'sam'));
select pg_temp.blocked('sales staff cannot create variants', format($q$insert into variants (org_id, product_id, sku, price_minor) values (%L, %L, 'SNK-1', 1)$q$, :'org_a', :'prod_a'));
select pg_temp.ok('sales staff cannot change prices (0 rows)', pg_temp.affected(format($q$update variants set price_minor = 1 where id = %L$q$, :'var_a')) = 0);
select pg_temp.blocked('sales staff cannot write stock movements', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 5000, 'purchase', %L)$q$, :'org_a', :'var_a', :'sam'));
select pg_temp.blocked('sales staff cannot write costs', format($q$insert into variant_costs (variant_id, org_id, cost_minor) values (%L, %L, 1) on conflict do nothing$q$, :'var_a', :'org_a'));
select pg_temp.as_ctx(:'rob', :'org_a');
select pg_temp.ok('read-only sees catalogue but no costs', (select count(*) from variants) = 1 and (select count(*) from variant_costs) = 0);
select pg_temp.blocked('read-only cannot adjust stock', format($q$insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values (%L, %L, -1000, 'damage', 'x', %L)$q$, :'org_a', :'var_a', :'rob'));

-- accountant: sees costs, cannot change anything
select pg_temp.as_ctx(:'acc', :'org_a');
select pg_temp.ok('accountant can see cost prices', (select cost_minor from variant_costs) = 1200000);
select pg_temp.ok('accountant cannot change costs (0 rows)', pg_temp.affected(format($q$update variant_costs set cost_minor = 1 where variant_id = %L$q$, :'var_a')) = 0);
select pg_temp.blocked('accountant cannot adjust stock', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 1000, 'purchase', %L)$q$, :'org_a', :'var_a', :'acc'));
-- manager can manage
select pg_temp.as_ctx(:'mia', :'org_a');
select pg_temp.ok('manager can change prices', pg_temp.affected(format($q$update variants set price_minor = 1700000 where id = %L$q$, :'var_a')) = 1);

-- stock integrity
select pg_temp.as_ctx(:'ivy', :'org_a');
insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values (:'org_a', :'var_a', -3000, 'damage', 'water damage', :'ivy');
select pg_temp.ok('balance follows movements', (select on_hand from stock_levels where variant_id = :'var_a') = 7000 and (select max(resulting_on_hand) from stock_movements where delta = -3000) = 7000);
select pg_temp.blocked('negative stock is impossible', format($q$insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values (%L, %L, -8000, 'adjustment', 'too many', %L)$q$, :'org_a', :'var_a', :'ivy'));
select pg_temp.ok('a rejected movement changes nothing', (select on_hand from stock_levels where variant_id = :'var_a') = 7000 and (select count(*) from stock_movements where variant_id = :'var_a') = 2);
select pg_temp.blocked('stock_levels cannot be edited directly', format($q$update stock_levels set on_hand = 999 where variant_id = %L$q$, :'var_a'));
select pg_temp.blocked('stock_levels cannot be inserted directly', format($q$insert into stock_levels (variant_id, org_id, on_hand) values (gen_random_uuid(), %L, 5)$q$, :'org_a'));
select pg_temp.blocked('movements cannot be updated', $q$update stock_movements set delta = 100$q$);
select pg_temp.blocked('movements cannot be deleted', $q$delete from stock_movements$q$);
select pg_temp.blocked('movements cannot be truncated', $q$truncate stock_movements$q$);
select pg_temp.blocked('zero-quantity movements are rejected', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 0, 'adjustment', %L)$q$, :'org_a', :'var_a', :'ivy'));
select pg_temp.blocked('unknown reasons are rejected', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 1000, 'gift', %L)$q$, :'org_a', :'var_a', :'ivy'));
select pg_temp.blocked('the sale reason is reserved for the sales module', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, -1000, 'sale', %L)$q$, :'org_a', :'var_a', :'ivy'));
select pg_temp.blocked('cannot attribute a movement to someone else', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 1000, 'purchase', %L)$q$, :'org_a', :'var_a', :'alice'));
select pg_temp.ok('balance equals the sum of movements (invariant)', (select on_hand from stock_levels where variant_id = :'var_a') = (select sum(delta) from stock_movements where variant_id = :'var_a'));

-- uniqueness and composite tenant keys
select pg_temp.blocked('SKU is unique within an organization', format($q$insert into variants (org_id, product_id, sku, price_minor) values (%L, %L, 'ric-050', 1)$q$, :'org_a', :'prod_a'));
select pg_temp.blocked('barcode is unique within an organization', format($q$insert into variants (org_id, product_id, sku, barcode, price_minor) values (%L, %L, 'OTHER-1', '6001234567890', 1)$q$, :'org_a', :'prod_a'));
select pg_temp.blocked('invalid SKU characters are rejected', format($q$insert into variants (org_id, product_id, sku, price_minor) values (%L, %L, 'bad sku!', 1)$q$, :'org_a', :'prod_a'));
select pg_temp.blocked('negative prices are rejected', format($q$insert into variants (org_id, product_id, sku, price_minor) values (%L, %L, 'NEG-1', -1)$q$, :'org_a', :'prod_a'));

-- cross-tenant
select pg_temp.as_ctx(:'bob', :'org_b');
insert into categories (org_id, name) values (:'org_b', 'Groceries') returning id as cat_b \gset
insert into products (org_id, name, created_by) values (:'org_b', 'B Product', :'bob') returning id as prod_b \gset
insert into variants (org_id, product_id, sku, price_minor) values (:'org_b', :'prod_b', 'RIC-050', 100);
select pg_temp.ok('the same SKU and category name can exist in another organization', true);
select pg_temp.ok('org B sees none of org A inventory', (select count(*) from products where id = :'prod_a') = 0 and (select count(*) from variants where id = :'var_a') = 0 and (select count(*) from stock_movements where variant_id = :'var_a') = 0 and (select count(*) from stock_levels where variant_id = :'var_a') = 0 and (select count(*) from variant_costs where variant_id = :'var_a') = 0);
select pg_temp.blocked('org B cannot attach a variant to org A''s product (composite key)', format($q$insert into variants (org_id, product_id, sku, price_minor) values (%L, %L, 'X-1', 1)$q$, :'org_b', :'prod_a'));
select pg_temp.blocked('org B cannot move stock of org A''s variant', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 5000, 'purchase', %L)$q$, :'org_b', :'var_a', :'bob'));
select pg_temp.blocked('org B cannot use org A''s category', format($q$insert into products (org_id, category_id, name, created_by) values (%L, %L, 'Y', %L)$q$, :'org_b', :'cat_a', :'bob'));
select pg_temp.blocked('org B cannot write into org A with a forged header', format($q$insert into products (org_id, name, created_by) values (%L, 'Z', %L)$q$, :'org_a', :'bob'));
select pg_temp.as_ctx(:'bob', :'org_a');
select pg_temp.ok('forged org context exposes no inventory', (select count(*) from products) = 0 and (select count(*) from variants) = 0);

-- audit
select pg_temp.as_ctx(:'alice', :'org_a');
select pg_temp.ok('stock adjustments are audited with the real actor', (select count(*) from audit_logs where org_id = :'org_a' and action = 'stock.adjust' and actor_user_id = :'ivy') = 1);
select pg_temp.ok('price changes are audited with before/after', (select (metadata->>'from_minor')::bigint = 1600000 and (metadata->>'to_minor')::bigint = 1700000 from audit_logs where org_id = :'org_a' and action = 'variant.price_change' and actor_user_id = :'mia'));
select pg_temp.ok('product and variant creation are audited', (select count(*) from audit_logs where org_id = :'org_a' and action in ('product.create','variant.create','variant.cost_change')) = 3);
select pg_temp.as_ctx(:'ivy', :'org_a');
select pg_temp.ok('inventory managers cannot read the audit log', (select count(*) from audit_logs) = 0);


-- ---------- 0009 fractional quantities (quantities are THOUSANDTHS of the unit) ----------
select pg_temp.as_ctx(:'ivy', :'org_a');
select pg_temp.ok('products default to whole-unit', (select allow_fractional from products where id = :'prod_a') = false);
insert into products (org_id, name, unit, allow_fractional, created_by) values (:'org_a', 'Loose rice', 'kg', true, :'ivy') returning id as prod_f \gset
insert into variants (org_id, product_id, sku, price_minor, reorder_level) values (:'org_a', :'prod_f', 'RICE-KG', 100000, 500) returning id as var_f \gset
insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (:'org_a', :'var_f', 2500, 'opening', :'ivy');
select pg_temp.ok('a fractional product accepts 2.5 kg', (select on_hand from stock_levels where variant_id = :'var_f') = 2500);
insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values (:'org_a', :'var_f', -750, 'damage', 'spilled', :'ivy');
select pg_temp.ok('fractional withdrawal is exact (2.500 - 0.750 = 1.750)', (select on_hand from stock_levels where variant_id = :'var_f') = 1750 and (select resulting_on_hand from stock_movements where delta = -750) = 1750);
select pg_temp.blocked('fractional product still cannot go negative', format($q$insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values (%L, %L, -1751, 'adjustment', 'x', %L)$q$, :'org_a', :'var_f', :'ivy'));
select pg_temp.ok('balance equals the sum of movements for the fractional product', (select on_hand from stock_levels where variant_id = :'var_f') = (select sum(delta) from stock_movements where variant_id = :'var_f'));
select pg_temp.blocked('a whole-unit product rejects +0.5', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 500, 'purchase', %L)$q$, :'org_a', :'var_a', :'ivy'));
select pg_temp.blocked('a whole-unit product rejects +1.5', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 1500, 'purchase', %L)$q$, :'org_a', :'var_a', :'ivy'));
select pg_temp.blocked('a whole-unit product rejects -0.001', format($q$insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values (%L, %L, -1, 'damage', 'x', %L)$q$, :'org_a', :'var_a', :'ivy'));
select pg_temp.ok('rejected fractional movements changed nothing', (select on_hand from stock_levels where variant_id = :'var_a') = 7000);
select pg_temp.blocked('the app role cannot turn fractions ON for an existing product', format($q$update products set allow_fractional = true where id = %L$q$, :'prod_a'));
select pg_temp.blocked('the app role cannot turn fractions OFF for an existing product', format($q$update products set allow_fractional = false where id = %L$q$, :'prod_f'));
select pg_temp.blocked('delta above the cap is rejected', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 100000000001, 'purchase', %L)$q$, :'org_a', :'var_f', :'ivy'));
select pg_temp.blocked('negative reorder level is rejected', format($q$update variants set reorder_level = -1 where id = %L$q$, :'var_f'));
select pg_temp.blocked('reorder level above the cap is rejected', format($q$update variants set reorder_level = 100000000001 where id = %L$q$, :'var_f'));
select pg_temp.as_ctx(:'alice', :'org_a');
select pg_temp.ok('fractional stock audit rows carry the quantity scale', (select count(*) from audit_logs where org_id = :'org_a' and action = 'stock.adjust' and (metadata->>'qty_scale')::int = 1000) >= 1);
select pg_temp.as_ctx(:'bob', :'org_b');
select pg_temp.blocked('org B cannot move org A fractional stock', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 500, 'purchase', %L)$q$, :'org_b', :'var_f', :'bob'));
select pg_temp.blocked('org B cannot mint a fractional movement against org A through its own org id', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (%L, %L, 500, 'purchase', %L)$q$, :'org_a', :'var_f', :'bob'));


-- ---------- 0010 business types, capability flags and the deletable sample catalogue ----------
select pg_temp.as_ctx(:'alice', null);
select app.create_organization('Org S','org-s','electronics') as org_s \gset
select app.create_organization('Org M','org-m','building_materials') as org_m \gset
select pg_temp.as_ctx(:'bob', null);
select app.create_organization('Org T','org-t','fashion') as org_t \gset
select pg_temp.as_ctx(:'alice', :'org_s');
insert into memberships (org_id, user_id, role) values (:'org_s', :'ivy', 'inventory_manager'), (:'org_s', :'sam', 'sales_staff'), (:'org_s', :'rob', 'read_only'), (:'org_s', :'acc', 'accountant'), (:'org_s', :'mia', 'manager'), (:'org_s', :'dave', 'administrator');

-- business types: 12 offered, 4 legacy kept valid but hidden; flags live in the database and the app role cannot edit them
select pg_temp.ok('12 business types are offered and 4 legacy ones stay valid but hidden', (select count(*) from business_types where selectable) = 12 and (select count(*) from business_types where not selectable) = 4);
select pg_temp.ok('capability flags follow the business type', (select tracks_expiry from business_types where code = 'pharmacy') and (select sold_by_weight from business_types where code = 'building_materials') and (select sold_by_weight from business_types where code = 'supermarket') and (select has_tables from business_types where code = 'restaurant') and (select has_bookings from business_types where code = 'services') and not (select tracks_expiry from business_types where code = 'electronics'));
select pg_temp.blocked('app role cannot edit capability flags', $q$update business_types set tracks_expiry = true where code = 'electronics'$q$);
select pg_temp.blocked('app role cannot add business types', $q$insert into business_types (code, label) values ('crypto_shop', 'Crypto shop')$q$);
select pg_temp.blocked('an unknown business type cannot be given to an organization', format($q$update organizations set business_type = 'cryptozoology' where id = %L$q$, :'org_s'));

-- the reference data holds COST prices: no role may read it directly
select pg_temp.as_ctx(:'sam', :'org_s');
select pg_temp.blocked('sales staff cannot read the sample reference data (it holds costs)', $q$select * from sample_catalogue_items$q$);
select pg_temp.as_ctx(:'alice', :'org_s');
select pg_temp.blocked('even the owner cannot read the sample reference data directly', $q$select * from sample_catalogue_items$q$);

-- organization phone: optional, shape-checked, owner/administrator only
select pg_temp.ok('owner can set the phone number', pg_temp.affected(format($q$update organizations set phone = '+234 803 123 4567' where id = %L$q$, :'org_s')) = 1);
select pg_temp.blocked('a malformed phone number is rejected', format($q$update organizations set phone = 'call me maybe' where id = %L$q$, :'org_s'));
select pg_temp.as_ctx(:'ivy', :'org_s');
select pg_temp.ok('an inventory manager cannot change the phone number (RLS hides the row)', pg_temp.affected(format($q$update organizations set phone = '08000000000' where id = %L$q$, :'org_s')) = 0);

-- role matrix for load / delete
select pg_temp.as_ctx(:'sam', :'org_s');
select pg_temp.blocked('sales staff cannot load samples', format($q$select app.load_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.blocked('sales staff cannot delete samples', format($q$select app.delete_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.as_ctx(:'rob', :'org_s');
select pg_temp.blocked('read-only users cannot load samples', format($q$select app.load_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.as_ctx(:'acc', :'org_s');
select pg_temp.blocked('accountants cannot load samples', format($q$select app.load_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.blocked('accountants cannot delete samples', format($q$select app.delete_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.as_ctx(:'ivy', :'org_s');
select pg_temp.ok('the rejected attempts created nothing', (select count(*) from products where sample_key is not null) = 0);

-- load: allowed for an inventory manager, idempotent
select (app.load_sample_catalogue(:'org_s')->>'loaded')::int as n1 \gset
select (app.load_sample_catalogue(:'org_s')->>'loaded')::int as n2 \gset
select pg_temp.ok('an inventory manager can load the electronics samples (5)', :n1 = 5);
select pg_temp.ok('loading twice does not duplicate', :n2 = 0 and (select count(*) from products where sample_key is not null) = 5 and (select count(*) from variants where sku like 'SMP-ELECTRONICS-%') = 5);
select pg_temp.ok('samples are flagged and carry opening stock', (select count(*) from stock_movements where is_sample and reason = 'opening') = 5 and (select count(*) from stock_levels where on_hand > 0) >= 5);
select pg_temp.ok('manager-level roles see sample costs (inventory manager)', (select count(*) from variant_costs vc join variants v on v.id = vc.variant_id where v.sku like 'SMP-%') = 5);

-- profit visibility: Sales Staff and Read Only never see sample costs, accountants do
select pg_temp.as_ctx(:'sam', :'org_s');
select pg_temp.ok('sales staff see sample products but zero costs', (select count(*) from products where sample_key is not null) = 5 and (select count(*) from variant_costs) = 0);
select pg_temp.as_ctx(:'rob', :'org_s');
select pg_temp.ok('read-only users see zero costs', (select count(*) from variant_costs) = 0);
select pg_temp.as_ctx(:'acc', :'org_s');
select pg_temp.ok('accountants can see sample costs', (select count(*) from variant_costs) = 5);

-- status: counts only, any member
select pg_temp.as_ctx(:'sam', :'org_s');
select pg_temp.ok('any member can read the status counts', (app.sample_catalogue_status(:'org_s')->>'present')::int = 5 and (app.sample_catalogue_status(:'org_s')->>'loadable')::int = 0 and (app.sample_catalogue_status(:'org_s')->>'deletable')::int = 5);

-- cross-tenant
select pg_temp.as_ctx(:'bob', :'org_t');
select pg_temp.blocked('org B cannot load samples into org S by passing its id', format($q$select app.load_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.blocked('org B cannot delete org S samples', format($q$select app.delete_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.blocked('org B cannot read org S sample status', format($q$select app.sample_catalogue_status(%L)$q$, :'org_s'));
select pg_temp.as_ctx(:'bob', :'org_s');
select pg_temp.blocked('a forged org context cannot load samples', format($q$select app.load_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.blocked('a forged org context cannot delete samples', format($q$select app.delete_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.as_ctx(null, null);
select pg_temp.blocked('no context: cannot load samples', format($q$select app.load_sample_catalogue(%L)$q$, :'org_s'));
select pg_temp.as_ctx(:'bob', :'org_t');
select (app.load_sample_catalogue(:'org_t')->>'loaded')::int as nt \gset
select pg_temp.ok('org B loads only its own (fashion) samples', :nt = 5 and (select count(*) from products where sample_key is not null) = 5 and (select count(*) from products where sample_key like 'electronics-%') = 0);
select pg_temp.ok('org B still sees none of org S inventory', (select count(*) from products where org_id = :'org_s') = 0);

-- fractional vs whole-unit samples (building materials: sold by bag, weight or roll)
select pg_temp.as_ctx(:'alice', :'org_m');
select (app.load_sample_catalogue(:'org_m')->>'loaded')::int as nm \gset
select pg_temp.ok('building materials load 6 samples', :nm = 6);
select pg_temp.ok('sand by the tonne is fractional and exact (12.500)', (select p.allow_fractional and sl.on_hand = 12500 from stock_levels sl join variants v on v.id = sl.variant_id join products p on p.id = v.product_id where v.sku = 'SMP-BUILDING-MATERIALS-02'));
select pg_temp.ok('nails by the kilo are fractional and exact (60.250)', (select p.allow_fractional and sl.on_hand = 60250 from stock_levels sl join variants v on v.id = sl.variant_id join products p on p.id = v.product_id where v.sku = 'SMP-BUILDING-MATERIALS-05'));
select pg_temp.ok('cement by the bag is whole-unit', (select not p.allow_fractional and sl.on_hand = 200000 from stock_levels sl join variants v on v.id = sl.variant_id join products p on p.id = v.product_id where v.sku = 'SMP-BUILDING-MATERIALS-01'));
select pg_temp.ok('every whole-unit sample has whole-unit stock and reorder level', (select count(*) from stock_levels sl join variants v on v.id = sl.variant_id join products p on p.id = v.product_id where p.org_id = :'org_m' and not p.allow_fractional and (sl.on_hand % 1000 <> 0 or v.reorder_level % 1000 <> 0)) = 0);
select pg_temp.ok('every sample balance equals the sum of its movements', (select count(*) from stock_levels sl where sl.org_id = :'org_m' and sl.on_hand <> (select coalesce(sum(m.delta), 0) from stock_movements m where m.variant_id = sl.variant_id)) = 0);

-- a legacy business type has no sample set: loading is a harmless no-op
select pg_temp.as_ctx(:'alice', :'org_a');
select pg_temp.ok('a legacy business type loads nothing', (app.load_sample_catalogue(:'org_a')->>'loaded')::int = 0);

-- forging the flags is impossible for the app role
select pg_temp.as_ctx(:'ivy', :'org_s');
insert into products (org_id, name, created_by) values (:'org_s', 'Real Item', :'ivy') returning id as prod_real \gset
insert into variants (org_id, product_id, sku, price_minor) values (:'org_s', :'prod_real', 'REAL-1', 5000) returning id as var_real \gset
insert into variant_costs (variant_id, org_id, cost_minor) values (:'var_real', :'org_s', 3000);
insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (:'org_s', :'var_real', 4000, 'opening', :'ivy');
select pg_temp.blocked('app role cannot create a product with a sample_key', format($q$insert into products (org_id, name, created_by, sample_key) values (%L, 'Forged', %L, 'forged-01')$q$, :'org_s', :'ivy'));
select pg_temp.blocked('app role cannot flag a real product as a sample', format($q$update products set sample_key = 'forged-02' where id = %L$q$, :'prod_real'));
select pg_temp.blocked('app role cannot clear the sample flag', $q$update products set sample_key = null where sample_key is not null$q$);
select pg_temp.blocked('app role cannot create a flagged stock movement', format($q$insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id, is_sample) values (%L, %L, 1000, 'purchase', %L, true)$q$, :'org_s', :'var_real', :'ivy'));
select set_config('app.sample_purge', 'on', false);
select pg_temp.blocked('the purge flag does not let the app role delete sample movements', $q$delete from stock_movements where is_sample$q$);
select pg_temp.blocked('the purge flag does not let the app role delete real movements', format($q$delete from stock_movements where variant_id = %L$q$, :'var_real'));
select set_config('app.sample_purge', 'off', false);
select pg_temp.blocked('app role cannot delete sample variants directly', $q$delete from variants where sku like 'SMP-%'$q$);
select pg_temp.blocked('app role cannot delete sample products directly', $q$delete from products where sample_key is not null$q$);
select pg_temp.blocked('real movements stay immutable', format($q$update stock_movements set delta = 9000 where variant_id = %L$q$, :'var_real'));

-- delete: only untouched samples go; a sample with REAL stock history is kept; real products are never touched
select v.id as v_keep from variants v where v.org_id = :'org_s' and v.sku = 'SMP-ELECTRONICS-01' \gset
insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values (:'org_s', :'v_keep', 5000, 'purchase', :'ivy');
select pg_temp.ok('a real purchase on a sample is recorded exactly', (select on_hand from stock_levels where variant_id = :'v_keep') = 13000);
select pg_temp.as_ctx(:'ivy', :'org_s');
select pg_temp.ok('status shows 4 deletable and 1 kept', (app.sample_catalogue_status(:'org_s')->>'deletable')::int = 4 and (app.sample_catalogue_status(:'org_s')->>'keptWithHistory')::int = 1);
select (d->>'deleted')::int as nd, jsonb_array_length(d->'kept') as nk, d->'kept'->0->>'name' as kept_name from (select app.delete_sample_catalogue(:'org_s') as d) x \gset
select pg_temp.ok('delete removed the 4 untouched samples and kept the one with real history', :nd = 4 and :nk = 1 and :'kept_name' = 'Smartphone 64GB');
select pg_temp.ok('the kept sample keeps its exact balance', (select on_hand from stock_levels where variant_id = :'v_keep') = 13000);
select pg_temp.ok('the real product, its cost and its stock are untouched', (select count(*) from products where id = :'prod_real') = 1 and (select on_hand from stock_levels where variant_id = :'var_real') = 4000 and (select cost_minor from variant_costs where variant_id = :'var_real') = 3000);
select pg_temp.ok('only the kept sample remains flagged', (select count(*) from products where org_id = :'org_s' and sample_key is not null) = 1 and (select count(*) from products where org_id = :'org_s') = 2);
select pg_temp.ok('deleted samples left no stock rows behind', (select count(*) from stock_levels where org_id = :'org_s') = 2 and (select count(*) from stock_movements where org_id = :'org_s') = 3);
select (d->>'deleted')::int as nd2, jsonb_array_length(d->'kept') as nk2 from (select app.delete_sample_catalogue(:'org_s') as d) x \gset
select pg_temp.ok('a second delete changes nothing', :nd2 = 0 and :nk2 = 1);
select pg_temp.as_ctx(:'alice', :'org_m');
select pg_temp.ok('org M still has all 6 samples', (select count(*) from products where org_id = :'org_m' and sample_key is not null) = 6);

-- reload after delete brings back only the missing samples (the kept one is not duplicated)
select pg_temp.as_ctx(:'ivy', :'org_s');
select (app.load_sample_catalogue(:'org_s')->>'loaded')::int as n3 \gset
select pg_temp.ok('reload restores the 4 deleted samples without duplicating the kept one', :n3 = 4 and (select count(*) from products where org_id = :'org_s' and sample_key is not null) = 5);

-- a real item that already uses a sample SKU is never overwritten: that sample is skipped
select pg_temp.as_ctx(:'bob', :'org_t');
insert into products (org_id, name, created_by) values (:'org_t', 'My own jeans', :'bob') returning id as prod_own \gset
select app.delete_sample_catalogue(:'org_t');
select pg_temp.ok('org B samples deleted; org B real product kept', (select count(*) from products where org_id = :'org_t') = 1 and (select name from products where id = :'prod_own') = 'My own jeans');
insert into variants (org_id, product_id, sku, price_minor) values (:'org_t', :'prod_own', 'SMP-FASHION-01', 100);
select (d->>'loaded')::int as n4, (d->>'skippedSkuTaken')::int as s4 from (select app.load_sample_catalogue(:'org_t') as d) x \gset
select pg_temp.ok('a SKU already used by a real item is skipped, not overwritten', :n4 = 4 and :s4 = 1 and (select price_minor from variants where sku = 'SMP-FASHION-01') = 100 and (select sample_key from products where id = :'prod_own') is null);

-- audit trail
select pg_temp.as_ctx(:'alice', :'org_s');
select pg_temp.ok('loads and deletes are audited with the real actor', (select count(*) from audit_logs where org_id = :'org_s' and action = 'catalogue.sample_load' and actor_user_id = :'ivy') = 2 and (select count(*) from audit_logs where org_id = :'org_s' and action = 'catalogue.sample_delete' and actor_user_id = :'ivy' and (metadata->>'deleted')::int = 4 and (metadata->>'kept')::int = 1) = 1);

\echo ALL RLS ATTACK TESTS PASSED
