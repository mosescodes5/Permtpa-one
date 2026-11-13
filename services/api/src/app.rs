use crate::{config::{Config, Environment}, mailer::Mailer, security, session};
use axum::{Json, Router, extract::State, http::{HeaderName, HeaderValue, StatusCode}, middleware, routing::{delete, get, patch, post}};
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
    pub cfg: Arc<Config>,
    pub mailer: Mailer,
    /// Verified against when the email is unknown, so unknown and known accounts cost the same time.
    pub dummy_hash: Arc<String>,
}

impl AppState {
    pub fn new(pool: PgPool, redis: Arc<redis::Client>, cfg: Config, mailer: Mailer) -> Self {
        let dummy_hash = Arc::new(security::hash_password("dummy-password-for-timing-equalisation").expect("argon2 hashing works"));
        Self { pool, redis, cfg: Arc::new(cfg), mailer, dummy_hash }
    }
}

const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

pub fn router(state: AppState) -> Router {
    let hdr = |name: &'static str, value: &'static str| SetResponseHeaderLayer::overriding(HeaderName::from_static(name), HeaderValue::from_static(value));

    let mut v1 = Router::new()
        .route("/v1/auth/register", post(crate::auth::register))
        .route("/v1/auth/verify-email", post(crate::auth::verify_email))
        .route("/v1/auth/resend-verification", post(crate::auth::resend_verification))
        .route("/v1/auth/login", post(crate::auth::login))
        .route("/v1/auth/logout", post(crate::auth::logout))
        .route("/v1/auth/forgot-password", post(crate::auth::forgot_password))
        .route("/v1/auth/reset-password", post(crate::auth::reset_password))
        .route("/v1/auth/csrf", get(crate::auth::csrf))
        .route("/v1/auth/sessions", get(crate::auth::list_sessions))
        .route("/v1/auth/sessions/revoke-others", post(crate::auth::revoke_other_sessions))
        .route("/v1/auth/sessions/{id}", delete(crate::auth::revoke_session))
        .route("/v1/auth/mfa/setup", post(crate::mfa::setup))
        .route("/v1/auth/mfa/enable", post(crate::mfa::enable))
        .route("/v1/auth/mfa/disable", post(crate::mfa::disable))
        .route("/v1/me", get(crate::auth::me))
        .route("/v1/organizations", post(crate::orgs::create))
        .route("/v1/orgs/{org_id}", get(crate::team::org_get).patch(crate::team::org_update))
        .route("/v1/orgs/{org_id}/members", get(crate::orgs::members))
        .route("/v1/orgs/{org_id}/members/{user_id}", patch(crate::team::member_update).delete(crate::team::member_remove))
        .route("/v1/orgs/{org_id}/invitations", get(crate::team::list_invitations).post(crate::team::invite))
        .route("/v1/orgs/{org_id}/invitations/{id}", delete(crate::team::revoke_invitation))
        .route("/v1/invitations/accept", post(crate::team::accept_invitation));
    // The development outbox exists ONLY in development (404 everywhere else — see tests).
    if state.cfg.environment == Environment::Development {
        v1 = v1.route("/v1/dev/outbox", get(dev_outbox));
    }
    let v1 = v1.layer(middleware::from_fn_with_state(state.clone(), session::csrf_guard));

    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .merge(v1)
        .with_state(state)
        .layer(hdr("x-content-type-options", "nosniff"))
        .layer(hdr("cache-control", "no-store"))
        .layer(hdr("referrer-policy", "no-referrer"))
        .layer(hdr("content-security-policy", "default-src 'none'; frame-ancestors 'none'"))
        .layer(RequestBodyLimitLayer::new(64 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::new(REQUEST_ID))
        .layer(SetRequestIdLayer::new(REQUEST_ID, MakeRequestUuid))
}

async fn dev_outbox(State(s): State<AppState>) -> Json<Value> {
    Json(json!({ "emails": s.mailer.outbox().iter().map(|e| json!({ "to": e.to, "subject": e.subject, "text": e.text })).collect::<Vec<_>>() }))
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
