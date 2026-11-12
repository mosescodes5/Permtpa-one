use axum::{Json, Router, extract::State, http::{HeaderName, HeaderValue, StatusCode}, routing::get};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tower_http::{
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub redis: Arc<redis::Client>,
}

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

pub fn router(state: AppState) -> Router {
    let hdr = |name: &'static str, value: &'static str| {
        SetResponseHeaderLayer::overriding(HeaderName::from_static(name), HeaderValue::from_static(value))
    };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .with_state(state)
        .layer(hdr("x-content-type-options", "nosniff"))
        .layer(hdr("cache-control", "no-store"))
        .layer(hdr("referrer-policy", "no-referrer"))
        .layer(hdr("content-security-policy", "default-src 'none'; frame-ancestors 'none'"))
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(REQUEST_ID))
        .layer(SetRequestIdLayer::new(REQUEST_ID, MakeRequestUuid))
}

/// Liveness: the process is up. Does not touch dependencies.
async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "permtpa-one-api", "version": env!("CARGO_PKG_VERSION") }))
}

/// Readiness: dependencies respond. Failure detail is logged, never returned to the caller.
async fn readyz(State(s): State<AppState>) -> (StatusCode, Json<Value>) {
    let timeout = Duration::from_secs(2);
    let db_ok = matches!(tokio::time::timeout(timeout, sqlx::query("select 1").execute(&s.pool)).await, Ok(Ok(_)));
    let redis_ok = tokio::time::timeout(timeout, async {
        let mut conn = s.redis.get_multiplexed_async_connection().await.ok()?;
        redis::cmd("PING").query_async::<String>(&mut conn).await.ok()
    })
    .await
    .ok()
    .flatten()
    .is_some();
    if !db_ok {
        tracing::error!("readiness: database check failed");
    }
    if !redis_ok {
        tracing::error!("readiness: redis check failed");
    }
    let status = if db_ok && redis_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    let label = |ok: bool| if ok { "ok" } else { "unavailable" };
    (status, Json(json!({ "status": if status == StatusCode::OK { "ready" } else { "not_ready" }, "checks": { "database": label(db_ok), "redis": label(redis_ok) } })))
}
