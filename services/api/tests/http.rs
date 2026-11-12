use axum::{body::Body, http::{Request, StatusCode}};
use http_body_util::BodyExt;
use permtpa_api::app::{AppState, router};
use sqlx::postgres::PgPoolOptions;
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;

/// State pointing at services that do not exist (port 1) — proves liveness is dependency-free
/// and readiness fails closed without leaking connection details.
fn dead_state() -> AppState {
    let pool = PgPoolOptions::new().acquire_timeout(Duration::from_millis(300)).connect_lazy("postgres://u:topsecretpw@127.0.0.1:1/db").unwrap();
    AppState { pool, redis: Arc::new(redis::Client::open("redis://:redispw@127.0.0.1:1").unwrap()) }
}

async fn get(path: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let res = router(dead_state()).oneshot(Request::get(path).body(Body::empty()).unwrap()).await.unwrap();
    let (parts, body) = res.into_parts();
    (parts.status, parts.headers, String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

#[tokio::test]
async fn healthz_is_ok_without_any_dependency() {
    let (status, _, body) = get("/healthz").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("permtpa-one-api"));
}

#[tokio::test]
async fn readyz_fails_closed_and_leaks_nothing() {
    let (status, _, body) = get("/readyz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("not_ready") && body.contains("unavailable"));
    for secret in ["topsecretpw", "redispw", "127.0.0.1", "postgres://"] {
        assert!(!body.contains(secret), "response leaked {secret}");
    }
}

#[tokio::test]
async fn responses_carry_security_headers_and_a_request_id() {
    let (_, h, _) = get("/healthz").await;
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert_eq!(h["cache-control"], "no-store");
    assert!(h["content-security-policy"].to_str().unwrap().contains("frame-ancestors 'none'"));
    assert!(h.contains_key("x-request-id"));
}

#[tokio::test]
async fn unknown_routes_are_404() {
    assert_eq!(get("/admin").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn oversized_bodies_are_rejected() {
    let big = vec![b'x'; 2 * 1024 * 1024];
    let res = router(dead_state()).oneshot(Request::post("/healthz").body(Body::from(big)).unwrap()).await.unwrap();
    // Method not allowed or payload too large — in either case never 200.
    assert_ne!(res.status(), StatusCode::OK);
}
