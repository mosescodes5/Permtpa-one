-- PERMTPA ONE — 0007 team management: accepting invitations, automatic audit trail for membership/org changes.

-- Only one PENDING invitation per (org, email). Older/expired ones are revoked by the API before re-inviting.
create unique index invitations_one_pending on invitations (org_id, email) where accepted_at is null and revoked_at is null;

-- Accepting an invitation. The caller must be authenticated, and their VERIFIED email must equal the invited email,
-- so a forwarded or leaked link is useless to anyone else. Atomic: only one redemption can succeed.
create function app.accept_invitation(p_token_hash bytea) returns uuid
language plpgsql security definer set search_path = public, pg_temp as $$
declare v_user uuid := app.current_user_id(); v_email citext; v_inv record;
begin
  if v_user is null then raise exception 'authentication required' using errcode = '42501'; end if;
  select email into v_email from users where id = v_user and status = 'active' and email_verified_at is not null;
  if v_email is null then return null; end if;

  update invitations set accepted_at = now()
   where token_hash = p_token_hash and accepted_at is null and revoked_at is null and expires_at > now() and email = v_email
   returning org_id, role, invited_by into v_inv;
  if v_inv.org_id is null then return null; end if;

  insert into memberships (org_id, user_id, role) values (v_inv.org_id, v_user, v_inv.role)
    on conflict (org_id, user_id) do nothing;
  insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
    values (v_inv.org_id, v_user, 'invitation.accepted', 'user', v_user::text, jsonb_build_object('role', v_inv.role));
  return v_inv.org_id;
end $$;
revoke all on function app.accept_invitation(bytea) from public;
grant execute on function app.accept_invitation(bytea) to permtpa_app;

-- Automatic audit trail. Triggers run as the table owner so audit rows cannot be skipped or forged by the caller;
-- the actor is whoever the transaction context says (never client-supplied).
create function app.audit_membership() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if tg_op = 'INSERT' then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (new.org_id, app.current_user_id(), 'member.add', 'user', new.user_id::text, jsonb_build_object('role', new.role));
  elsif tg_op = 'UPDATE' and (old.role is distinct from new.role or old.status is distinct from new.status) then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (new.org_id, app.current_user_id(),
              case when old.role is distinct from new.role then 'member.role_change' else 'member.status_change' end,
              'user', new.user_id::text,
              jsonb_build_object('from_role', old.role, 'to_role', new.role, 'from_status', old.status, 'to_status', new.status));
  elsif tg_op = 'DELETE' then
    -- skip when the whole organization is being removed (cascade)
    if exists (select 1 from organizations o where o.id = old.org_id) then
      insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
        values (old.org_id, app.current_user_id(), 'member.remove', 'user', old.user_id::text, jsonb_build_object('role', old.role));
    end if;
  end if;
  return null;
end $$;
create trigger memberships_audit after insert or update or delete on memberships for each row execute function app.audit_membership();

create function app.audit_invitation() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if tg_op = 'INSERT' then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (new.org_id, app.current_user_id(), 'invitation.create', 'invitation', new.id::text, jsonb_build_object('role', new.role));
  elsif tg_op = 'UPDATE' and old.revoked_at is null and new.revoked_at is not null then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id)
      values (new.org_id, app.current_user_id(), 'invitation.revoke', 'invitation', new.id::text);
  end if;
  return null;
end $$;
create trigger invitations_audit after insert or update on invitations for each row execute function app.audit_invitation();

create function app.audit_org() returns trigger language plpgsql security definer set search_path = public, pg_temp as $$
begin
  if old.name is distinct from new.name or old.business_type is distinct from new.business_type then
    insert into audit_logs (org_id, actor_user_id, action, target_type, target_id, metadata)
      values (new.id, app.current_user_id(), 'org.update', 'organization', new.id::text,
              jsonb_build_object('from_name', old.name, 'to_name', new.name, 'from_type', old.business_type, 'to_type', new.business_type));
  end if;
  return null;
end $$;
create trigger organizations_audit after update on organizations for each row execute function app.audit_org();
