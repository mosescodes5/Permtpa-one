//! Inventory: categories, products, variants, costs, stock movements, history, lookup, low-stock and valuation.
//! Stock balances change ONLY by inserting movements (the database trigger applies them atomically and refuses negatives).
use crate::{
    app::AppState,
    error::ApiError,
    session::{ApiJson, AuthSession},
    team::{map_db, org_tx},
};
use axum::{Json, extract::{Path, Query, State}, http::StatusCode};
use permtpa_domain::OrgRole;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

type Tx = Transaction<'static, Postgres>;
const MAX_MINOR: i64 = 1_000_000_000_000;
const MAX_QTY: i32 = 100_000_000;

fn require_manage(role: OrgRole) -> Result<(), ApiError> {
    if role.can_manage_inventory() { Ok(()) } else { Err(ApiError::Forbidden { code: "forbidden" }) }
}

// ------------------------------------------------------------------ validation
fn text(raw: &str, min: usize, max: usize, what: &'static str) -> Result<String, ApiError> {
    let t = raw.trim();
    if (min..=max).contains(&t.chars().count()) && !t.chars().any(char::is_control) { Ok(t.to_owned()) } else { Err(ApiError::bad("invalid_value", format!("{what} must be {min}–{max} characters."))) }
}
fn sku(raw: &str) -> Result<String, ApiError> {
    let t = raw.trim();
    let ok = !t.is_empty() && t.len() <= 40 && t.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()) && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok { Ok(t.to_owned()) } else { Err(ApiError::bad("invalid_sku", "SKU may contain letters, digits, '.', '_' and '-' (max 40).")) }
}
fn barcode(raw: &str) -> Result<String, ApiError> {
    let t = raw.trim();
    if (4..=32).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') { Ok(t.to_owned()) } else { Err(ApiError::bad("invalid_barcode", "Barcode must be 4–32 letters, digits or '-'.")) }
}
fn money(v: i64, what: &'static str) -> Result<i64, ApiError> {
    if (0..=MAX_MINOR).contains(&v) { Ok(v) } else { Err(ApiError::bad("invalid_amount", format!("{what} is out of range."))) }
}
fn qty(v: i32, what: &'static str) -> Result<i32, ApiError> {
    if (0..=MAX_QTY).contains(&v) { Ok(v) } else { Err(ApiError::bad("invalid_quantity", format!("{what} is out of range."))) }
}

// ------------------------------------------------------------------ categories
pub async fn categories_list(State(s): State<AppState>, sess: AuthSession, Path(org): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    let (mut tx, _) = org_tx(&s, &sess, org).await?;
    let rows: Vec<(Uuid, String, i64)> = sqlx::query_as("select c.id, c.name::text, count(p.id) from categories c left join products p on p.category_id = c.id and p.org_id = c.org_id where c.org_id = $1 group by c.id order by lower(c.name::text)").bind(org).fetch_all(&mut *tx).await?;
    tx.rollback().await?;
    Ok(Json(json!({ "categories": rows.into_iter().map(|(id, name, n)| json!({ "id": id, "name": name, "productCount": n })).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategoryReq {
    name: String,
}

pub async fn category_create(State(s): State<AppState>, sess: AuthSession, Path(org): Path<Uuid>, ApiJson(req): ApiJson<CategoryReq>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let name = text(&req.name, 1, 80, "Category name")?;
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let id: Uuid = sqlx::query_scalar("insert into categories (org_id, name) values ($1, $2) returning id").bind(org).bind(&name).fetch_one(&mut *tx).await.map_err(map_db)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id, "name": name, "productCount": 0 }))))
}

pub async fn category_update(State(s): State<AppState>, sess: AuthSession, Path((org, id)): Path<(Uuid, Uuid)>, ApiJson(req): ApiJson<CategoryReq>) -> Result<Json<Value>, ApiError> {
    let name = text(&req.name, 1, 80, "Category name")?;
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let n = sqlx::query("update categories set name = $3 where id = $1 and org_id = $2").bind(id).bind(org).bind(&name).execute(&mut *tx).await.map_err(map_db)?.rows_affected();
    tx.commit().await?;
    if n == 0 { return Err(ApiError::NotFound); }
    Ok(Json(json!({ "id": id, "name": name })))
}

pub async fn category_delete(State(s): State<AppState>, sess: AuthSession, Path((org, id)): Path<(Uuid, Uuid)>) -> Result<StatusCode, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    match sqlx::query("delete from categories where id = $1 and org_id = $2").bind(id).bind(org).execute(&mut *tx).await {
        Ok(r) if r.rows_affected() == 1 => { tx.commit().await?; Ok(StatusCode::NO_CONTENT) }
        Ok(_) => Err(ApiError::NotFound),
        Err(sqlx::Error::Database(d)) if d.code().as_deref() == Some("23503") => Err(ApiError::Conflict { code: "category_in_use" }),
        Err(e) => Err(map_db(e)),
    }
}

// ------------------------------------------------------------------ products
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NewVariant {
    name: Option<String>,
    sku: String,
    barcode: Option<String>,
    price_minor: i64,
    cost_minor: Option<i64>,
    reorder_level: Option<i32>,
    opening_stock: Option<i32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NewProduct {
    name: String,
    description: Option<String>,
    category_id: Option<Uuid>,
    unit: Option<String>,
    variants: Vec<NewVariant>,
}

async fn insert_variant(tx: &mut Tx, org: Uuid, user: Uuid, product: Uuid, v: &NewVariant) -> Result<Uuid, ApiError> {
    let name = v.name.as_deref().map(|n| text(n, 1, 80, "Variant name")).transpose()?.unwrap_or_else(|| "Default".into());
    let sku = sku(&v.sku)?;
    let barcode = v.barcode.as_deref().map(barcode).transpose()?;
    let price = money(v.price_minor, "Price")?;
    let reorder = qty(v.reorder_level.unwrap_or(0), "Reorder level")?;
    let id: Uuid = sqlx::query_scalar("insert into variants (org_id, product_id, name, sku, barcode, price_minor, reorder_level) values ($1, $2, $3, $4::citext, $5, $6, $7) returning id")
        .bind(org).bind(product).bind(name).bind(&sku).bind(barcode).bind(price).bind(reorder).fetch_one(&mut **tx).await.map_err(map_db)?;
    if let Some(c) = v.cost_minor {
        sqlx::query("insert into variant_costs (variant_id, org_id, cost_minor) values ($1, $2, $3)").bind(id).bind(org).bind(money(c, "Cost")?).execute(&mut **tx).await.map_err(map_db)?;
    }
    if let Some(o) = v.opening_stock.filter(|o| *o != 0) {
        sqlx::query("insert into stock_movements (org_id, variant_id, delta, reason, actor_user_id) values ($1, $2, $3, 'opening', $4)").bind(org).bind(id).bind(qty(o, "Opening stock")?).bind(user).execute(&mut **tx).await.map_err(map_db)?;
    }
    Ok(id)
}

pub async fn product_create(State(s): State<AppState>, sess: AuthSession, Path(org): Path<Uuid>, ApiJson(req): ApiJson<NewProduct>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let name = text(&req.name, 1, 160, "Product name")?;
    let description = req.description.as_deref().map(|d| text(d, 0, 2000, "Description")).transpose()?;
    let unit = req.unit.as_deref().map(|u| text(u, 1, 20, "Unit")).transpose()?.unwrap_or_else(|| "pcs".into());
    if req.variants.is_empty() || req.variants.len() > 50 {
        return Err(ApiError::bad("invalid_variants", "Provide between 1 and 50 variants."));
    }
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let pid: Uuid = sqlx::query_scalar("insert into products (org_id, category_id, name, description, unit, created_by) values ($1, $2, $3, $4, $5, $6) returning id")
        .bind(org).bind(req.category_id).bind(name).bind(description).bind(unit).bind(sess.user_id).fetch_one(&mut *tx).await.map_err(map_db)?;
    for v in &req.variants {
        insert_variant(&mut tx, org, sess.user_id, pid, v).await?;
    }
    let item = fetch_products(&mut tx, org, &Filter { id: Some(pid), ..Filter::default() }).await?.0.into_iter().next().ok_or(ApiError::Internal)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[derive(Default)]
struct Filter {
    id: Option<Uuid>,
    search: Option<String>,
    category: Option<Uuid>,
    status: Option<String>,
    limit: i64,
    offset: i64,
}

/// One round trip per page: variants are aggregated in SQL (no N+1). Cost columns are NULL for roles RLS hides them from.
async fn fetch_products(tx: &mut Tx, org: Uuid, f: &Filter) -> Result<(Vec<Value>, i64), ApiError> {
    let search = f.search.as_ref().map(|s| s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let status = f.status.clone().unwrap_or_else(|| "active".into());
    let limit = if f.limit <= 0 { 50 } else { f.limit.min(100) };
    const WHERE: &str = "p.org_id = $1 and ($2::uuid is null or p.id = $2) and ($3::uuid is null or p.category_id = $3) and ($4 = 'all' or p.status = $4)
        and ($5::text is null or p.name ilike '%' || $5 || '%' or exists (select 1 from variants v2 where v2.product_id = p.id and v2.org_id = p.org_id and (v2.sku::text ilike $5 || '%' or v2.barcode = $6)))";
    let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("select count(*) from products p where {WHERE}"))).bind(org).bind(f.id).bind(f.category).bind(&status).bind(&search).bind(f.search.as_deref()).fetch_one(&mut **tx).await?;
    let sql = format!(
        "select p.id, p.name, p.description, p.unit, p.status, p.category_id, c.name::text,
            coalesce(json_agg(json_build_object('id', v.id, 'name', v.name, 'sku', v.sku::text, 'barcode', v.barcode, 'priceMinor', v.price_minor, 'costMinor', vc.cost_minor,
                'reorderLevel', v.reorder_level, 'status', v.status, 'onHand', coalesce(sl.on_hand, 0)) order by v.created_at, v.id) filter (where v.id is not null), '[]'::json)::text
         from products p
         left join categories c on c.id = p.category_id and c.org_id = p.org_id
         left join variants v on v.product_id = p.id and v.org_id = p.org_id
         left join variant_costs vc on vc.variant_id = v.id
         left join stock_levels sl on sl.variant_id = v.id
         where {WHERE}
         group by p.id, c.name order by lower(p.name), p.id limit {limit} offset {}", f.offset.clamp(0, 100_000));
    type Row = (Uuid, String, Option<String>, String, String, Option<Uuid>, Option<String>, String);
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(org).bind(f.id).bind(f.category).bind(&status).bind(&search).bind(f.search.as_deref()).fetch_all(&mut **tx).await?;
    let items = rows.into_iter().map(|(id, name, desc, unit, st, cat, cat_name, variants)| {
        json!({ "id": id, "name": name, "description": desc, "unit": unit, "status": st, "categoryId": cat, "category": cat_name, "variants": serde_json::from_str::<Value>(&variants).unwrap_or(Value::Null) })
    }).collect();
    Ok((items, total))
}

#[derive(Deserialize)]
pub struct ListQuery {
    search: Option<String>,
    category: Option<Uuid>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

pub async fn product_list(State(s): State<AppState>, sess: AuthSession, Path(org): Path<Uuid>, Query(q): Query<ListQuery>) -> Result<Json<Value>, ApiError> {
    if let Some(st) = q.status.as_deref() {
        if !matches!(st, "active" | "archived" | "all") { return Err(ApiError::bad("invalid_status", "Status must be active, archived or all.")); }
    }
    let search = q.search.as_deref().map(str::trim).filter(|x| !x.is_empty()).map(|x| x.chars().take(100).collect::<String>());
    let (mut tx, _) = org_tx(&s, &sess, org).await?;
    let f = Filter { id: None, search, category: q.category, status: q.status, limit: q.limit.unwrap_or(50), offset: q.offset.unwrap_or(0) };
    let (items, total) = fetch_products(&mut tx, org, &f).await?;
    tx.rollback().await?;
    Ok(Json(json!({ "items": items, "total": total, "limit": if f.limit <= 0 { 50 } else { f.limit.min(100) }, "offset": f.offset.clamp(0, 100_000) })))
}

pub async fn product_get(State(s): State<AppState>, sess: AuthSession, Path((org, id)): Path<(Uuid, Uuid)>) -> Result<Json<Value>, ApiError> {
    let (mut tx, _) = org_tx(&s, &sess, org).await?;
    let (items, _) = fetch_products(&mut tx, org, &Filter { id: Some(id), status: Some("all".into()), ..Filter::default() }).await?;
    tx.rollback().await?;
    items.into_iter().next().map(Json).ok_or(ApiError::NotFound)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateProduct {
    name: Option<String>,
    description: Option<String>,
    category_id: Option<Uuid>,
    clear_category: Option<bool>,
    unit: Option<String>,
    status: Option<String>,
}

pub async fn product_update(State(s): State<AppState>, sess: AuthSession, Path((org, id)): Path<(Uuid, Uuid)>, ApiJson(req): ApiJson<UpdateProduct>) -> Result<Json<Value>, ApiError> {
    let name = req.name.as_deref().map(|n| text(n, 1, 160, "Product name")).transpose()?;
    let description = req.description.as_deref().map(|d| text(d, 0, 2000, "Description")).transpose()?;
    let unit = req.unit.as_deref().map(|u| text(u, 1, 20, "Unit")).transpose()?;
    if let Some(st) = req.status.as_deref() {
        if st != "active" && st != "archived" { return Err(ApiError::bad("invalid_status", "Status must be active or archived.")); }
    }
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let exists: bool = sqlx::query_scalar("select exists (select 1 from products where id = $1 and org_id = $2)").bind(id).bind(org).fetch_one(&mut *tx).await?;
    if !exists { return Err(ApiError::NotFound); }
    let n = sqlx::query("update products set name = coalesce($3, name), description = coalesce($4, description), category_id = case when $5 then null else coalesce($6, category_id) end, unit = coalesce($7, unit), status = coalesce($8, status) where id = $1 and org_id = $2")
        .bind(id).bind(org).bind(name).bind(description).bind(req.clear_category.unwrap_or(false)).bind(req.category_id).bind(unit).bind(req.status).execute(&mut *tx).await.map_err(map_db)?.rows_affected();
    if n == 0 { return Err(ApiError::Forbidden { code: "forbidden" }); }
    let item = fetch_products(&mut tx, org, &Filter { id: Some(id), status: Some("all".into()), ..Filter::default() }).await?.0.into_iter().next().ok_or(ApiError::Internal)?;
    tx.commit().await?;
    Ok(Json(item))
}

// ------------------------------------------------------------------ variants
pub async fn variant_create(State(s): State<AppState>, sess: AuthSession, Path((org, product)): Path<(Uuid, Uuid)>, ApiJson(req): ApiJson<NewVariant>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let exists: bool = sqlx::query_scalar("select exists (select 1 from products where id = $1 and org_id = $2)").bind(product).bind(org).fetch_one(&mut *tx).await?;
    if !exists { return Err(ApiError::NotFound); }
    insert_variant(&mut tx, org, sess.user_id, product, &req).await?;
    let item = fetch_products(&mut tx, org, &Filter { id: Some(product), status: Some("all".into()), ..Filter::default() }).await?.0.into_iter().next().ok_or(ApiError::Internal)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateVariant {
    name: Option<String>,
    sku: Option<String>,
    barcode: Option<String>,
    clear_barcode: Option<bool>,
    price_minor: Option<i64>,
    cost_minor: Option<i64>,
    reorder_level: Option<i32>,
    status: Option<String>,
}

pub async fn variant_update(State(s): State<AppState>, sess: AuthSession, Path((org, id)): Path<(Uuid, Uuid)>, ApiJson(req): ApiJson<UpdateVariant>) -> Result<Json<Value>, ApiError> {
    let name = req.name.as_deref().map(|n| text(n, 1, 80, "Variant name")).transpose()?;
    let sku = req.sku.as_deref().map(sku).transpose()?;
    let barcode = req.barcode.as_deref().map(barcode).transpose()?;
    let price = req.price_minor.map(|p| money(p, "Price")).transpose()?;
    let cost = req.cost_minor.map(|c| money(c, "Cost")).transpose()?;
    let reorder = req.reorder_level.map(|r| qty(r, "Reorder level")).transpose()?;
    if let Some(st) = req.status.as_deref() {
        if st != "active" && st != "archived" { return Err(ApiError::bad("invalid_status", "Status must be active or archived.")); }
    }
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let product: Option<Uuid> = sqlx::query_scalar("select product_id from variants where id = $1 and org_id = $2").bind(id).bind(org).fetch_optional(&mut *tx).await?;
    let Some(product) = product else { return Err(ApiError::NotFound) };
    let n = sqlx::query("update variants set name = coalesce($3, name), sku = coalesce($4::citext, sku), barcode = case when $5 then null else coalesce($6, barcode) end, price_minor = coalesce($7, price_minor), reorder_level = coalesce($8, reorder_level), status = coalesce($9, status) where id = $1 and org_id = $2")
        .bind(id).bind(org).bind(name).bind(sku).bind(req.clear_barcode.unwrap_or(false)).bind(barcode).bind(price).bind(reorder).bind(req.status).execute(&mut *tx).await.map_err(map_db)?.rows_affected();
    if n == 0 { return Err(ApiError::Forbidden { code: "forbidden" }); }
    if let Some(c) = cost {
        sqlx::query("insert into variant_costs (variant_id, org_id, cost_minor) values ($1, $2, $3) on conflict (variant_id) do update set cost_minor = excluded.cost_minor, updated_at = now()").bind(id).bind(org).bind(c).execute(&mut *tx).await.map_err(map_db)?;
    }
    let item = fetch_products(&mut tx, org, &Filter { id: Some(product), status: Some("all".into()), ..Filter::default() }).await?.0.into_iter().next().ok_or(ApiError::Internal)?;
    tx.commit().await?;
    Ok(Json(item))
}

// ------------------------------------------------------------------ stock movements
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockReq {
    delta: i32,
    reason: String,
    note: Option<String>,
}

pub async fn stock_adjust(State(s): State<AppState>, sess: AuthSession, Path((org, variant)): Path<(Uuid, Uuid)>, ApiJson(req): ApiJson<StockReq>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let reason = req.reason.as_str();
    if !matches!(reason, "purchase" | "return_in" | "adjustment" | "damage" | "correction") {
        return Err(ApiError::bad("invalid_reason", "Choose purchase, return_in, adjustment, damage or correction."));
    }
    if req.delta == 0 || req.delta.abs() > MAX_QTY { return Err(ApiError::bad("invalid_quantity", "Quantity must be non-zero and within range.")); }
    if matches!(reason, "purchase" | "return_in") && req.delta < 0 { return Err(ApiError::bad("invalid_quantity", "This reason adds stock; use a positive quantity.")); }
    if reason == "damage" && req.delta > 0 { return Err(ApiError::bad("invalid_quantity", "Damage removes stock; use a negative quantity.")); }
    let note = req.note.as_deref().map(|n| text(n, 0, 200, "Note")).transpose()?.filter(|n| !n.is_empty());
    if matches!(reason, "adjustment" | "damage" | "correction") && note.is_none() {
        return Err(ApiError::bad("note_required", "Explain why you are changing the stock."));
    }
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    require_manage(role)?;
    let exists: bool = sqlx::query_scalar("select exists (select 1 from variants where id = $1 and org_id = $2)").bind(variant).bind(org).fetch_one(&mut *tx).await?;
    if !exists { return Err(ApiError::NotFound); }
    let (id, on_hand): (i64, i32) = sqlx::query_as("insert into stock_movements (org_id, variant_id, delta, reason, note, actor_user_id) values ($1, $2, $3, $4, $5, $6) returning id, resulting_on_hand")
        .bind(org).bind(variant).bind(req.delta).bind(reason).bind(note).bind(sess.user_id).fetch_one(&mut *tx).await.map_err(map_db)?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({ "movementId": id, "onHand": on_hand }))))
}

#[derive(Deserialize)]
pub struct MovementsQuery {
    limit: Option<i64>,
    before: Option<i64>,
}

pub async fn movements(State(s): State<AppState>, sess: AuthSession, Path((org, variant)): Path<(Uuid, Uuid)>, Query(q): Query<MovementsQuery>) -> Result<Json<Value>, ApiError> {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let (mut tx, _) = org_tx(&s, &sess, org).await?;
    let exists: bool = sqlx::query_scalar("select exists (select 1 from variants where id = $1 and org_id = $2)").bind(variant).bind(org).fetch_one(&mut *tx).await?;
    if !exists { return Err(ApiError::NotFound); }
    type Row = (i64, i32, String, Option<String>, Option<i32>, Uuid, Option<String>, String);
    let rows: Vec<Row> = sqlx::query_as("select m.id, m.delta, m.reason, m.note, m.resulting_on_hand, m.actor_user_id, u.full_name, m.created_at::text from stock_movements m left join users u on u.id = m.actor_user_id where m.org_id = $1 and m.variant_id = $2 and ($3::bigint is null or m.id < $3) order by m.id desc limit $4")
        .bind(org).bind(variant).bind(q.before).bind(limit + 1).fetch_all(&mut *tx).await?;
    tx.rollback().await?;
    let more = rows.len() as i64 > limit;
    let page: Vec<_> = rows.into_iter().take(limit as usize).collect();
    let next = if more { page.last().map(|r| r.0) } else { None };
    Ok(Json(json!({
        "movements": page.into_iter().map(|(id, delta, reason, note, after, actor, name, at)| json!({ "id": id, "delta": delta, "reason": reason, "note": note, "resultingOnHand": after, "actor": { "id": actor, "name": name }, "createdAt": at })).collect::<Vec<_>>(),
        "nextBefore": next
    })))
}

// ------------------------------------------------------------------ lookup, low stock, valuation
#[derive(Deserialize)]
pub struct LookupQuery {
    barcode: Option<String>,
    sku: Option<String>,
}

pub async fn lookup(State(s): State<AppState>, sess: AuthSession, Path(org): Path<Uuid>, Query(q): Query<LookupQuery>) -> Result<Json<Value>, ApiError> {
    let (barcode, sku) = (q.barcode.as_deref().map(str::trim), q.sku.as_deref().map(str::trim));
    if barcode.is_none() == sku.is_none() { return Err(ApiError::bad("invalid_lookup", "Provide exactly one of barcode or sku.")); }
    let (mut tx, _) = org_tx(&s, &sess, org).await?;
    let row: Option<(Uuid, Uuid, String, String, String, i64, Option<i64>, i32)> = sqlx::query_as(
        "select v.id, p.id, p.name, v.name, v.sku::text, v.price_minor, vc.cost_minor, coalesce(sl.on_hand, 0) from variants v join products p on p.id = v.product_id and p.org_id = v.org_id
         left join variant_costs vc on vc.variant_id = v.id left join stock_levels sl on sl.variant_id = v.id
         where v.org_id = $1 and v.status = 'active' and p.status = 'active' and (($2::text is not null and v.barcode = $2) or ($3::text is not null and v.sku = $3::citext))")
        .bind(org).bind(barcode).bind(sku).fetch_optional(&mut *tx).await?;
    tx.rollback().await?;
    let (vid, pid, pname, vname, vsku, price, cost, on_hand) = row.ok_or(ApiError::NotFound)?;
    Ok(Json(json!({ "variantId": vid, "productId": pid, "productName": pname, "variantName": vname, "sku": vsku, "priceMinor": price, "costMinor": cost, "onHand": on_hand })))
}

pub async fn low_stock(State(s): State<AppState>, sess: AuthSession, Path(org): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    let (mut tx, _) = org_tx(&s, &sess, org).await?;
    let rows: Vec<(Uuid, Uuid, String, String, String, i32, i32)> = sqlx::query_as(
        "select v.id, p.id, p.name, v.name, v.sku::text, coalesce(sl.on_hand, 0), v.reorder_level from variants v join products p on p.id = v.product_id and p.org_id = v.org_id left join stock_levels sl on sl.variant_id = v.id
         where v.org_id = $1 and v.status = 'active' and p.status = 'active' and v.reorder_level > 0 and coalesce(sl.on_hand, 0) <= v.reorder_level
         order by coalesce(sl.on_hand, 0)::float8 / v.reorder_level, lower(p.name) limit 100")
        .bind(org).fetch_all(&mut *tx).await?;
    tx.rollback().await?;
    Ok(Json(json!({ "items": rows.into_iter().map(|(vid, pid, pname, vname, sku, on_hand, reorder)| json!({ "variantId": vid, "productId": pid, "productName": pname, "variantName": vname, "sku": sku, "onHand": on_hand, "reorderLevel": reorder })).collect::<Vec<_>>() })))
}

/// Valuation = Σ on_hand × cost, computed with exact arithmetic in the database (NUMERIC) and returned as a string.
pub async fn valuation(State(s): State<AppState>, sess: AuthSession, Path(org): Path<Uuid>) -> Result<Json<Value>, ApiError> {
    let (mut tx, role) = org_tx(&s, &sess, org).await?;
    if !role.can_view_costs() { return Err(ApiError::Forbidden { code: "forbidden" }); }
    let (total, units, uncosted): (String, i64, i64) = sqlx::query_as(
        "select coalesce(sum(coalesce(sl.on_hand, 0)::numeric * vc.cost_minor), 0)::text, coalesce(sum(coalesce(sl.on_hand, 0)), 0)::bigint, count(*) filter (where vc.variant_id is null and coalesce(sl.on_hand, 0) > 0)
         from variants v join products p on p.id = v.product_id and p.org_id = v.org_id left join variant_costs vc on vc.variant_id = v.id left join stock_levels sl on sl.variant_id = v.id
         where v.org_id = $1 and v.status = 'active' and p.status = 'active'").bind(org).fetch_one(&mut *tx).await?;
    tx.rollback().await?;
    Ok(Json(json!({ "totalMinor": total, "units": units, "variantsMissingCost": uncosted, "currency": "NGN" })))
}
