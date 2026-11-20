//! Business-type onboarding: the picker list (with capability flags) and the deletable sample catalogue.
//! Roles are enforced twice: an explicit check here (clear 403) and again inside the SECURITY DEFINER functions.
//! Sample rows are flagged in the database (`products.sample_key`, `stock_movements.is_sample`); the app role can neither
//! create nor change those flags, so nothing sent over HTTP can make a real product look like a sample.
use crate::{
    app::AppState,
    db::begin_tenant_tx,
    error::ApiError,
    session::AuthSession,
    team::{map_db, org_tx},
};
use axum::{
    Json,
    extract::{Path, State},
};
use permtpa_domain::OrgRole;
use serde_json::{Value, json};
use uuid::Uuid;

type TypeRow = (String, String, String, bool, bool, bool, bool);

/// Capability flags exactly as stored in `business_types` (tracks_expiry, sold_by_weight, has_tables, has_bookings).
pub fn capabilities(flags: (bool, bool, bool, bool)) -> Value {
    let (tracks_expiry, sold_by_weight, has_tables, has_bookings) = flags;
    json!({
        "tracksExpiry": tracks_expiry,
        "soldByWeight": sold_by_weight,
        "hasTables": has_tables,
        "hasBookings": has_bookings
    })
}

fn require_manage(role: OrgRole) -> Result<(), ApiError> {
    if role.can_manage_inventory() {
        Ok(())
    } else {
        Err(ApiError::Forbidden { code: "forbidden" })
    }
}

fn parse(text: &str) -> Result<Value, ApiError> {
    serde_json::from_str(text).map_err(ApiError::internal)
}

/// The business types offered at sign-up, in display order. Any signed-in user may read them (no organization needed yet).
pub async fn business_types(
    State(s): State<AppState>,
    sess: AuthSession,
) -> Result<Json<Value>, ApiError> {
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let rows: Vec<TypeRow> = sqlx::query_as(
        "select code, label, description, tracks_expiry, sold_by_weight, has_tables, has_bookings from business_types where selectable order by sort_order, code",
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.rollback().await?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(
            |(code, label, description, expiry, weight, tables, bookings)| {
                json!({
                    "code": code,
                    "label": label,
                    "description": description,
                    "capabilities": capabilities((expiry, weight, tables, bookings))
                })
            },
        )
        .collect();
    Ok(Json(json!({ "items": items })))
}

/// Counts only (no costs). Any active member may read it so the UI can decide which buttons to show.
pub async fn sample_status(
    State(s): State<AppState>,
    sess: AuthSession,
    Path(org): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let (mut tx, _) = org_tx(&s, &sess, org).await?;
    let text: String = sqlx::query_scalar("select app.sample_catalogue_status($1)::text")
        .bind(org)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
    tx.rollback().await?;
    Ok(Json(parse(&text)?))
}

/// Idempotent: loading twice never duplicates. Samples whose SKU is already used by a real item are skipped and counted.
pub async fn sample_load(
    State(s): State<AppState>,
    sess: AuthSession,
    Path(org): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let text: String = sqlx::query_scalar("select app.load_sample_catalogue($1)::text")
        .bind(org)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
    tx.commit().await?;
    Ok(Json(parse(&text)?))
}

/// Removes only sample products with no real stock history. Samples that were ever received, sold or adjusted are KEPT and
/// listed under `kept`. Real products are never touched.
pub async fn sample_delete(
    State(s): State<AppState>,
    sess: AuthSession,
    Path(org): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let text: String = sqlx::query_scalar("select app.delete_sample_catalogue($1)::text")
        .bind(org)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db)?;
    tx.commit().await?;
    Ok(Json(parse(&text)?))
}

#[cfg(test)]
mod tests {
    use super::capabilities;

    #[test]
    fn capabilities_use_the_documented_keys() {
        let v = capabilities((true, false, true, false));
        assert_eq!(v["tracksExpiry"], true);
        assert_eq!(v["soldByWeight"], false);
        assert_eq!(v["hasTables"], true);
        assert_eq!(v["hasBookings"], false);
    }
}
