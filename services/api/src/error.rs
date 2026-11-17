use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;

/// RFC 9457 problem+json. Messages are deliberately generic; details go to logs, never to clients.
#[derive(Debug)]
pub enum ApiError {
    BadRequest { code: &'static str, detail: String },
    Unauthorized { code: &'static str },
    Forbidden { code: &'static str },
    NotFound,
    Conflict { code: &'static str },
    TooManyRequests { retry_after: u64 },
    Unavailable,
    Internal,
}

impl ApiError {
    pub fn bad(code: &'static str, detail: impl Into<String>) -> Self {
        Self::BadRequest {
            code,
            detail: detail.into(),
        }
    }
    pub fn unauthorized() -> Self {
        Self::Unauthorized {
            code: "unauthenticated",
        }
    }
    pub fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!(error = %err, "internal error");
        Self::Internal
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::internal(e)
    }
}
impl From<crate::security::CryptoError> for ApiError {
    fn from(e: crate::security::CryptoError) -> Self {
        Self::internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, title, detail, retry) = match self {
            Self::BadRequest { code, detail } => (
                StatusCode::BAD_REQUEST,
                code,
                "Invalid request",
                detail,
                None,
            ),
            Self::Unauthorized { code } => (
                StatusCode::UNAUTHORIZED,
                code,
                "Authentication required",
                "Authentication failed.".into(),
                None,
            ),
            Self::Forbidden { code } => (
                StatusCode::FORBIDDEN,
                code,
                "Forbidden",
                "You are not allowed to do this.".into(),
                None,
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Not found",
                "The resource was not found.".into(),
                None,
            ),
            Self::Conflict { code } => (
                StatusCode::CONFLICT,
                code,
                "Conflict",
                "The request conflicts with the current state.".into(),
                None,
            ),
            Self::TooManyRequests { retry_after } => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Too many requests",
                "Please try again later.".into(),
                Some(retry_after),
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Service unavailable",
                "Please try again shortly.".into(),
                None,
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal",
                "Internal error",
                "Something went wrong.".into(),
                None,
            ),
        };
        let mut res = (status, Json(json!({ "type": "about:blank", "title": title, "status": status.as_u16(), "code": code, "detail": detail }))).into_response();
        res.headers_mut().insert(
            header::CONTENT_TYPE,
            "application/problem+json".parse().expect("static header"),
        );
        if let Some(s) = retry {
            res.headers_mut().insert(
                header::RETRY_AFTER,
                s.to_string().parse().expect("numeric header"),
            );
        }
        res
    }
}
