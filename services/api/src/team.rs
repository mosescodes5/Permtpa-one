//! Team management: organization profile, invitations and member administration.
//! Authorization is enforced twice: explicit role checks here (clear 403s) and Row Level Security in the database.
use crate::{
    app::AppState,
    db::{TenantContext, begin_tenant_tx},
    error::ApiError,
    mailer::Email,
    ratelimit, security,
    session::{ApiJson, AuthSession},
    validate,
};
use axum::{Json, extract::{Path, State}, http::StatusCode};
use permtpa_domain::OrgRole;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

type Tx = Transaction<'static, Postgres>;

/// Enter an organization context and resolve the caller's role. A non-member gets 404 (indistinguishable from a missing org).
async fn org_tx(s: &AppState, sess: &AuthSession, org_id: Uuid) -> Result<(Tx, OrgRole), ApiError> {
    let mut tx = begin_tenant_tx(&s.pool, TenantContext { user_id: Some(sess.user_id), org_id: Some(org_id) }).await?;
    let role: Option<String> = sqlx::query_scalar("select app.current_role()::text").fetch_one(&mut *tx).await?;
    let role = role.ok_or(ApiError::NotFound)?.parse::<OrgRole>().map_err(ApiError::internal)?;
    Ok((tx, role))
}

fn require_admin(role: OrgRole) -> Result<(), ApiError> {
    if role.can_manage_members() { Ok(()) } else { Err(ApiError::Forbidden { code: "forbidden" }) }
}

/// Translate database rejections (RLS, triggers, constraints) into precise HTTP errors.
fn map_db(e: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(d) = &e {
        match d.code().as_deref() {
            Some("42501") => return ApiError::Forbidden { code: "forbidden" },
            Some("P0001") if d.message().contains("active owner") => return ApiError::Conflict { code: "last_owner" },
            Some("23505") => return ApiError::Conflict { code: "conflict" },
            Some("23503") => return ApiError::bad("invalid_reference", "Choose a valid value."),
            _ => {}
        }
    }
    ApiError::internal(e)
}

async fn profile(tx: &mut Tx, org_id: Uuid, role: OrgRole) -> Result<Value, ApiError> {
    let (id, name, slug, btype, currency): (Uuid, String, String, String, String) =
        sqlx::query_as("select id, name, slug::text, business_type, currency::text from organizations where id = $1").bind(org_id).fetch_one(&mut **tx).await?;
    Ok(json!({ "id": id, "name": name, "slug": slug, "businessType": btype, "currency": currency, "yourRole": role.as_str() }))
}

pub async fn org_get(State(s): State<AppState>, sess: AuthSession, Path(org_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org_id).await?;
    let p = profile(&mut tx, org_id, role).await?;
    tx.rollback().await?;
    Ok(Json(p))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateOrg {
    name: Option<String>,
    business_type: Option<String>,
}

pub async fn org_update(State(s): State<AppState>, sess: AuthSession, Path(org_id): Path<Uuid>, ApiJson(req): ApiJson<UpdateOrg>) -> Result<Json<Value>, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org_id).await?;
    require_admin(role)?;
    if req.name.is_none() && req.business_type.is_none() {
        return Err(ApiError::bad("nothing_to_update", "Provide a field to change."));
    }
    let name = req.name.as_deref().map(validate::org_name).transpose()?;
    let n = sqlx::query("update organizations set name = coalesce($2, name), business_type = coalesce($3, business_type) where id = $1")
        .bind(org_id).bind(name).bind(req.business_type).execute(&mut *tx).await.map_err(map_db)?.rows_affected();
    if n == 0 { return Err(ApiError::Forbidden { code: "forbidden" }); }
    let p = profile(&mut tx, org_id, role).await?;
    tx.commit().await?;
    Ok(Json(p))
}

// ------------------------------------------------------------------ invitations
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InviteReq {
    email: String,
    role: String,
}

pub async fn invite(State(s): State<AppState>, sess: AuthSession, Path(org_id): Path<Uuid>, ApiJson(req): ApiJson<InviteReq>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let email = validate::email(&req.email)?;
    let invited_role = req.role.parse::<OrgRole>().map_err(|_| ApiError::bad("invalid_role", "Choose a valid role."))?;
    if invited_role == OrgRole::Owner {
        return Err(ApiError::bad("invalid_role", "Ownership cannot be granted by invitation."));
    }
    let (mut tx, role) = org_tx(&s, &sess, org_id).await?;
    require_admin(role)?;
    ratelimit::hit(&s.redis, &format!("invite:{org_id}"), 30, 3600).await?;

    let already: bool = sqlx::query_scalar("select exists (select 1 from memberships m join users u on u.id = m.user_id where m.org_id = $1 and u.email = $2::citext)").bind(org_id).bind(&email).fetch_one(&mut *tx).await?;
    if already { return Err(ApiError::Conflict { code: "already_member" }); }

    // A fresh invitation replaces any earlier pending/expired one for the same address.
    sqlx::query("update invitations set revoked_at = now() where org_id = $1 and email = $2::citext and accepted_at is null and revoked_at is null").bind(org_id).bind(&email).execute(&mut *tx).await.map_err(map_db)?;
    let token = security::new_token()?;
    let hash = security::hash_token(&token);
    let (id, expires): (Uuid, String) = sqlx::query_as("insert into invitations (org_id, email, role, token_hash, invited_by, expires_at) values ($1, $2::citext, $3::org_role, $4, $5, now() + interval '7 days') returning id, expires_at::text")
        .bind(org_id).bind(&email).bind(invited_role.as_str()).bind(&hash[..]).bind(sess.user_id).fetch_one(&mut *tx).await.map_err(map_db)?;
    let org_name: String = sqlx::query_scalar("select name from organizations where id = $1").bind(org_id).fetch_one(&mut *tx).await?;
    let inviter: String = sqlx::query_scalar("select full_name from users where id = $1").bind(sess.user_id).fetch_one(&mut *tx).await?;
    tx.commit().await?;

    let text = format!("{inviter} invited you to join {org_name} on PERMTPA ONE as {}.\n\nAccept the invitation (log in or create an account with this email address first):\n{}/accept-invite?token={token}\n\nThis invitation expires in 7 days and only works for {email}.", role_label(invited_role), s.cfg.public_url);
    if let Err(e) = s.mailer.send(Email { to: email.clone(), subject: format!("You're invited to {org_name} on PERMTPA ONE"), text }).await {
        tracing::error!(error = %e, "failed to send invitation email");
    }
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "email": email, "role": invited_role.as_str(), "expiresAt": expires }))))
}

fn role_label(r: OrgRole) -> String {
    r.as_str().replace('_', " ")
}

pub async fn list_invitations(State(s): State<AppState>, sess: AuthSession, Path(org_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org_id).await?;
    require_admin(role)?;
    let rows: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
        "select id, email::text, role::text, created_at::text, expires_at::text from invitations where org_id = $1 and accepted_at is null and revoked_at is null and expires_at > now() order by created_at desc",
    ).bind(org_id).fetch_all(&mut *tx).await?;
    tx.rollback().await?;
    Ok(Json(json!({ "invitations": rows.into_iter().map(|(id, email, role, created, expires)| json!({ "id": id, "email": email, "role": role, "createdAt": created, "expiresAt": expires })).collect::<Vec<_>>() })))
}

pub async fn revoke_invitation(State(s): State<AppState>, sess: AuthSession, Path((org_id, id)): Path<(Uuid, Uuid)>) -> Result<StatusCode, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org_id).await?;
    require_admin(role)?;
    let n = sqlx::query("update invitations set revoked_at = now() where id = $1 and org_id = $2 and accepted_at is null and revoked_at is null").bind(id).bind(org_id).execute(&mut *tx).await.map_err(map_db)?.rows_affected();
    tx.commit().await?;
    if n == 1 { Ok(StatusCode::NO_CONTENT) } else { Err(ApiError::NotFound) }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptReq {
    token: String,
}

pub async fn accept_invitation(State(s): State<AppState>, sess: AuthSession, ApiJson(req): ApiJson<AcceptReq>) -> Result<Json<Value>, ApiError> {
    ratelimit::hit(&s.redis, &format!("accept:{}", sess.user_id), 20, 3600).await?;
    let hash = security::hash_token(req.token.trim());
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let org: Option<Uuid> = sqlx::query_scalar("select app.accept_invitation($1)").bind(&hash[..]).fetch_one(&mut *tx).await.map_err(map_db)?;
    tx.commit().await?;
    match org {
        Some(id) => Ok(Json(json!({ "orgId": id }))),
        None => Err(ApiError::bad("invalid_token", "This invitation is invalid, expired, or was sent to a different email address.")),
    }
}

// ------------------------------------------------------------------ members
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateMember {
    role: Option<String>,
    status: Option<String>,
}

pub async fn member_update(State(s): State<AppState>, sess: AuthSession, Path((org_id, user_id)): Path<(Uuid, Uuid)>, ApiJson(req): ApiJson<UpdateMember>) -> Result<Json<Value>, ApiError> {
    if req.role.is_none() && req.status.is_none() {
        return Err(ApiError::bad("nothing_to_update", "Provide a role or status."));
    }
    let new_role = req.role.as_deref().map(|r| r.parse::<OrgRole>().map(|x| x.as_str()).map_err(|_| ApiError::bad("invalid_role", "Choose a valid role."))).transpose()?;
    if let Some(st) = req.status.as_deref() {
        if st != "active" && st != "suspended" { return Err(ApiError::bad("invalid_status", "Status must be active or suspended.")); }
    }
    let (mut tx, role) = org_tx(&s, &sess, org_id).await?;
    require_admin(role)?;
    let exists: bool = sqlx::query_scalar("select exists (select 1 from memberships where org_id = $1 and user_id = $2)").bind(org_id).bind(user_id).fetch_one(&mut *tx).await?;
    if !exists { return Err(ApiError::NotFound); }
    let n = sqlx::query("update memberships set role = coalesce($3::org_role, role), status = coalesce($4, status) where org_id = $1 and user_id = $2")
        .bind(org_id).bind(user_id).bind(new_role).bind(req.status.as_deref()).execute(&mut *tx).await.map_err(map_db)?.rows_affected();
    if n == 0 {
        // The row exists but RLS refused the change (e.g. an administrator touching an owner).
        return Err(ApiError::Forbidden { code: "forbidden" });
    }
    let (r, st): (String, String) = sqlx::query_as("select role::text, status from memberships where org_id = $1 and user_id = $2").bind(org_id).bind(user_id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "userId": user_id, "role": r, "status": st })))
}

pub async fn member_remove(State(s): State<AppState>, sess: AuthSession, Path((org_id, user_id)): Path<(Uuid, Uuid)>) -> Result<StatusCode, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org_id).await?;
    require_admin(role)?;
    let exists: bool = sqlx::query_scalar("select exists (select 1 from memberships where org_id = $1 and user_id = $2)").bind(org_id).bind(user_id).fetch_one(&mut *tx).await?;
    if !exists { return Err(ApiError::NotFound); }
    let n = sqlx::query("delete from memberships where org_id = $1 and user_id = $2").bind(org_id).bind(user_id).execute(&mut *tx).await.map_err(map_db)?.rows_affected();
    if n == 0 { return Err(ApiError::Forbidden { code: "forbidden" }); }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
