use crate::{
    app::AppState,
    db::{TenantContext, begin_tenant_tx},
    error::ApiError,
    security, validate,
    session::{ApiJson, AuthSession},
};
use axum::{Json, extract::{Path, State}, http::StatusCode};
use permtpa_domain::OrgRole;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateOrg {
    name: String,
    business_type: String,
}

/// Creating an organization is the ONLY way to become an Owner (enforced in the database).
pub async fn create(State(s): State<AppState>, sess: AuthSession, ApiJson(req): ApiJson<CreateOrg>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let name = validate::org_name(&req.name)?;
    let slug = format!("{}-{}", validate::slugify(&name), &security::base32(&security::random_bytes::<4>()?).to_lowercase()[..6]);
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let id: Result<Uuid, sqlx::Error> = sqlx::query_scalar("select app.create_organization($1, $2::citext, $3)").bind(&name).bind(&slug).bind(&req.business_type).fetch_one(&mut *tx).await;
    match id {
        Ok(id) => {
            tx.commit().await?;
            Ok((StatusCode::CREATED, Json(json!({ "id": id, "slug": slug, "name": name, "role": "owner" }))))
        }
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23503") => Err(ApiError::bad("invalid_business_type", "Choose a valid business type.")),
        Err(e) => Err(e.into()),
    }
}

/// The org id comes from the URL but is NEVER trusted: the database only honours it for an active member,
/// so a non-member (or a guessed id) sees exactly what a non-existent org looks like — 404.
pub async fn members(State(s): State<AppState>, sess: AuthSession, Path(org_id): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    let mut tx = begin_tenant_tx(&s.pool, TenantContext { user_id: Some(sess.user_id), org_id: Some(org_id) }).await?;
    let role: Option<String> = sqlx::query_scalar("select app.current_role()::text").fetch_one(&mut *tx).await?;
    let Some(role) = role else { return Err(ApiError::NotFound) };
    let can_see_email = role.parse::<OrgRole>().map(OrgRole::can_manage_members).unwrap_or(false);
    let rows: Vec<(Uuid, String, String, String, String)> = sqlx::query_as(
        "select m.user_id, u.full_name, u.email::text, m.role::text, m.status from memberships m join users u on u.id = m.user_id where m.org_id = $1 order by u.full_name",
    )
    .bind(org_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.rollback().await?;
    // Least privilege: only owners/administrators see other people's email addresses.
    let members: Vec<Value> = rows.into_iter().map(|(id, name, email, role, status)| json!({ "userId": id, "fullName": name, "email": can_see_email.then_some(email), "role": role, "status": status })).collect();
    Ok(Json(json!({ "yourRole": role, "members": members })))
}
