-- PERMTPA ONE — 0004 Row Level Security + least-privilege grants + immutability.
-- The runtime role (permtpa_app) is NOT the table owner, so RLS always applies to it. Deny by default.

alter table users         enable row level security;
alter table sessions      enable row level security;
alter table organizations enable row level security;
alter table memberships   enable row level security;
alter table invitations   enable row level security;
alter table audit_logs    enable row level security;

-- ---- users: self + members of the active org. Column-level grants hide password_hash entirely.
create policy users_select on users for select to permtpa_app
  using (id = app.current_user_id() or id in (select user_id from memberships where org_id = app.current_org_id()));
create policy users_update_self on users for update to permtpa_app
  using (id = app.current_user_id()) with check (id = app.current_user_id());
grant select (id, email, full_name, email_verified_at, status, mfa_enabled, created_at) on users to permtpa_app;
grant update (full_name) on users to permtpa_app;

-- ---- sessions: a user sees and revokes only their own.
create policy sessions_select on sessions for select to permtpa_app using (user_id = app.current_user_id());
create policy sessions_insert on sessions for insert to permtpa_app with check (user_id = app.current_user_id());
create policy sessions_update on sessions for update to permtpa_app using (user_id = app.current_user_id()) with check (user_id = app.current_user_id());
grant select, insert on sessions to permtpa_app;
grant update (last_seen_at, revoked_at) on sessions to permtpa_app;

-- ---- organizations: visible to members; editable by owner/admin of the active org. Created only via app.create_organization().
create policy orgs_select on organizations for select to permtpa_app
  using (id = app.current_org_id() or id in (select org_id from memberships where user_id = app.current_user_id()));
create policy orgs_update on organizations for update to permtpa_app
  using (id = app.current_org_id() and app.is_org_admin()) with check (id = app.current_org_id() and app.is_org_admin());
grant select on organizations to permtpa_app;
grant update (name, business_type, currency) on organizations to permtpa_app;

-- ---- memberships: a user sees their own memberships (to pick an org) and everyone in the active org.
create policy memberships_select on memberships for select to permtpa_app
  using (user_id = app.current_user_id() or org_id = app.current_org_id());
-- Only owners/administrators manage membership; only an owner may touch owner rows or grant ownership.
create policy memberships_insert on memberships for insert to permtpa_app
  with check (org_id = app.current_org_id() and app.is_org_admin() and (role <> 'owner' or app.current_role() = 'owner'));
create policy memberships_update on memberships for update to permtpa_app
  using (org_id = app.current_org_id() and app.is_org_admin() and (role <> 'owner' or app.current_role() = 'owner'))
  with check (org_id = app.current_org_id() and app.is_org_admin() and (role <> 'owner' or app.current_role() = 'owner'));
create policy memberships_delete on memberships for delete to permtpa_app
  using (org_id = app.current_org_id() and app.is_org_admin() and (role <> 'owner' or app.current_role() = 'owner'));
grant select, insert, delete on memberships to permtpa_app;
grant update (role, status) on memberships to permtpa_app;

-- Every organization must always keep at least one active owner.
create function app.guard_last_owner() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if old.role = 'owner' and old.status = 'active'
     and (tg_op = 'DELETE' or new.role <> 'owner' or new.status <> 'active')
     and not exists (select 1 from memberships m where m.org_id = old.org_id and m.id <> old.id and m.role = 'owner' and m.status = 'active') then
    -- allow when the whole organization is being deleted (cascade)
    if exists (select 1 from organizations o where o.id = old.org_id) then
      raise exception 'an organization must keep at least one active owner' using errcode = 'P0001';
    end if;
  end if;
  return coalesce(new, old);
end $$;
create trigger memberships_last_owner before update or delete on memberships for each row execute function app.guard_last_owner();

-- ---- invitations: owner/admin only.
create policy invitations_all on invitations for all to permtpa_app
  using (org_id = app.current_org_id() and app.is_org_admin())
  with check (org_id = app.current_org_id() and app.is_org_admin() and invited_by = app.current_user_id());
grant select, insert on invitations to permtpa_app;
grant update (revoked_at, accepted_at) on invitations to permtpa_app;

-- ---- audit_logs: append-only. Any active member may write for their org; only owner/admin may read.
create policy audit_select on audit_logs for select to permtpa_app using (org_id = app.current_org_id() and app.is_org_admin());
create policy audit_insert on audit_logs for insert to permtpa_app
  with check (org_id = app.current_org_id() and actor_user_id = app.current_user_id());
grant select, insert on audit_logs to permtpa_app;      -- NO update/delete/truncate privilege for the app role
grant usage on sequence audit_logs_id_seq to permtpa_app;

-- Defence in depth: even a privileged role cannot silently rewrite history.
create function app.audit_immutable() returns trigger language plpgsql as $$
begin raise exception 'audit_logs is append-only' using errcode = 'P0001'; end $$;
create trigger audit_no_update_delete before update or delete on audit_logs for each row execute function app.audit_immutable();
create trigger audit_no_truncate before truncate on audit_logs for each statement execute function app.audit_immutable();

grant select on business_types to permtpa_app;
