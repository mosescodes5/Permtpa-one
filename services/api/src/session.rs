//! Cookie sessions, client IP resolution, request extractors and the CSRF guard.
use crate::{app::AppState, config::Config, db::{TenantContext, begin_tenant_tx}, error::ApiError, security};
use axum::{
    Json,
    extract::{ConnectInfo, FromRequest, FromRequestParts, Request, State},
    http::{HeaderMap, Method, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use uuid::Uuid;

pub const SESSION_MAX_AGE_SECS: u64 = 30 * 24 * 3600;

pub fn cookie_name(cfg: &Config) -> &'static str {
    // `__Host-` requires Secure, so it is only used when cookies are Secure (non-development).
    if cfg.cookie_secure() { "__Host-permtpa_session" } else { "permtpa_session" }
}

pub fn set_cookie(cfg: &Config, token: &str) -> String {
    format!("{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}", cookie_name(cfg), token, SESSION_MAX_AGE_SECS, if cfg.cookie_secure() { "; Secure" } else { "" })
}

pub fn clear_cookie(cfg: &Config) -> String {
    format!("{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}", cookie_name(cfg), if cfg.cookie_secure() { "; Secure" } else { "" })
}

pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).flat_map(|v| v.split(';')).find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == name && !v.is_empty()).then(|| v.to_owned())
    })
}

/// JSON body extractor that returns our problem+json on malformed input.
pub struct ApiJson<T>(pub T);
impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(ApiJson(v)),
            Err(_) => Err(ApiError::bad("invalid_json", "The request body is invalid.")),
        }
    }
}

/// Client IP: the socket peer, or — only when TRUSTED_PROXY_HOPS > 0 — the entry that many hops from the
/// right of X-Forwarded-For (anything further left is attacker-controlled and ignored).
pub struct ClientIp(pub IpAddr);
impl FromRequestParts<AppState> for ClientIp {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let peer = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip()).unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        let hops = state.cfg.trusted_proxy_hops;
        if hops > 0 {
            let xff: Vec<&str> = parts.headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()).map(|v| v.split(',').map(str::trim).collect()).unwrap_or_default();
            if let Some(ip) = xff.len().checked_sub(hops).and_then(|i| xff.get(i)).and_then(|s| s.parse().ok()) {
                return Ok(ClientIp(ip));
            }
        }
        Ok(ClientIp(peer))
    }
}

pub fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|v| v.chars().take(500).collect())
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub token_hash: [u8; 32],
}

impl AuthSession {
    pub fn ctx(&self) -> TenantContext {
        TenantContext { user_id: Some(self.user_id), org_id: None }
    }
    pub fn csrf(&self, cfg: &Config) -> String {
        security::csrf_token(&cfg.csrf_key, &self.token_hash)
    }
}

/// Resolves the cookie to a live session (verified user, not revoked/expired, not idle > 7 days).
pub async fn resolve(state: &AppState, headers: &HeaderMap) -> Result<Option<AuthSession>, ApiError> {
    let Some(token) = read_cookie(headers, cookie_name(&state.cfg)) else { return Ok(None) };
    let hash = security::hash_token(&token);
    let row: Option<(Uuid, Uuid, bool, bool)> = sqlx::query_as(
        "select session_id, user_id, last_seen_at < now() - interval '7 days', last_seen_at < now() - interval '60 seconds' from app.resolve_session($1)",
    )
    .bind(&hash[..])
    .fetch_optional(&state.pool)
    .await?;
    let Some((session_id, user_id, idle_expired, stale)) = row else { return Ok(None) };
    let session = AuthSession { session_id, user_id, token_hash: hash };
    if idle_expired {
        revoke(state, &session, session.session_id).await?;
        return Ok(None);
    }
    if stale {
        let mut tx = begin_tenant_tx(&state.pool, session.ctx()).await?;
        sqlx::query("update sessions set last_seen_at = now() where id = $1").bind(session_id).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    Ok(Some(session))
}

pub async fn revoke(state: &AppState, by: &AuthSession, session_id: Uuid) -> Result<bool, ApiError> {
    let mut tx = begin_tenant_tx(&state.pool, by.ctx()).await?;
    let n = sqlx::query("update sessions set revoked_at = now() where id = $1 and revoked_at is null").bind(session_id).execute(&mut *tx).await?.rows_affected();
    tx.commit().await?;
    Ok(n == 1)
}

impl FromRequestParts<AppState> for AuthSession {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        resolve(state, &parts.headers).await?.ok_or_else(ApiError::unauthorized)
    }
}

/// Endpoints that run BEFORE a session exists. They are still protected by the Origin check
/// (login-CSRF), but cannot require a session-bound token.
const PRE_AUTH: &[&str] = &[
    "/v1/auth/login", "/v1/auth/register", "/v1/auth/verify-email", "/v1/auth/resend-verification", "/v1/auth/forgot-password", "/v1/auth/reset-password",
];

/// CSRF defence for every state-changing request:
///  1. `Origin` must be present and on the allow-list (blocks cross-site form posts and fetches).
///  2. If a session cookie is presented, `X-CSRF-Token` must equal the HMAC bound to that session.
pub async fn csrf_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(req).await;
    }
    let origin = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()).map(|o| o.trim_end_matches('/'));
    if !origin.is_some_and(|o| state.cfg.allowed_origins.iter().any(|a| a == o)) {
        return ApiError::Forbidden { code: "csrf_origin" }.into_response();
    }
    if !PRE_AUTH.contains(&req.uri().path()) {
        if let Some(token) = read_cookie(req.headers(), cookie_name(&state.cfg)) {
            let expected = security::csrf_token(&state.cfg.csrf_key, &security::hash_token(&token));
            let given = req.headers().get("x-csrf-token").and_then(|v| v.to_str().ok()).unwrap_or("");
            if !security::ct_eq(expected.as_bytes(), given.as_bytes()) {
                return ApiError::Forbidden { code: "csrf_token" }.into_response();
            }
        }
    }
    next.run(req).await
}
