use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use std::time::Duration;
use uuid::Uuid;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn connect(url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url)
        .await
}

/// Identity for one database transaction. Both values are resolved SERVER-SIDE (session cookie -> user,
/// verified membership -> org) and never taken from request bodies or headers.
#[derive(Debug, Clone, Copy, Default)]
pub struct TenantContext {
    pub user_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
}

/// Begins a transaction with the request context applied using `set_config(..., is_local => true)`.
/// Transaction-local settings are discarded at COMMIT/ROLLBACK, so context can never leak to the next
/// request that reuses the same pooled connection. PostgreSQL RLS policies read these settings.
pub async fn begin_tenant_tx(
    pool: &PgPool,
    ctx: TenantContext,
) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("select set_config('app.user_id', $1, true), set_config('app.org_id', $2, true)")
        .bind(ctx.user_id.map(|u| u.to_string()).unwrap_or_default())
        .bind(ctx.org_id.map(|o| o.to_string()).unwrap_or_default())
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
