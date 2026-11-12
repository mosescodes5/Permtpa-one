use crate::{
    app::AppState,
    db::{TenantContext, begin_tenant_tx},
    error::ApiError,
    mailer::Email,
    mfa, ratelimit, security,
    session::{self, ApiJson, AuthSession, ClientIp},
    validate,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const GENERIC_ACCEPTED: &str = "If the details are valid, we have sent you an email.";
const EMAIL_TOKEN_TTL: &str = "interval '24 hours'";

fn email_key(email: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(email.as_bytes()))
}

fn invalid_credentials() -> ApiError {
    ApiError::Unauthorized { code: "invalid_credentials" }
}

fn accepted() -> Response {
    (StatusCode::ACCEPTED, Json(json!({ "message": GENERIC_ACCEPTED }))).into_response()
}

async fn issue_token_and_mail(state: &AppState, user_id: Uuid, to: &str, purpose: &'static str) -> Result<(), ApiError> {
    let token = security::new_token()?;
    let hash = security::hash_token(&token);
    let mut tx = begin_tenant_tx(&state.pool, TenantContext::default()).await?;
    let sql = format!("select app.issue_email_token($1, $2, $3, {EMAIL_TOKEN_TTL})");
    sqlx::query(sqlx::AssertSqlSafe(sql)).bind(user_id).bind(purpose).bind(&hash[..]).execute(&mut *tx).await?;
    tx.commit().await?;
    let (subject, path, lead) = match purpose {
        "verify_email" => ("Verify your PERMTPA ONE email", "verify-email", "Welcome to PERMTPA ONE. Confirm your email address to activate your account:"),
        _ => ("Reset your PERMTPA ONE password", "reset-password", "We received a request to reset your password. Use this link to choose a new one:"),
    };
    let text = format!("{lead}\n\n{}/{path}?token={token}\n\nThis link expires in 24 hours and can be used once. If you did not request it, you can ignore this email.", state.cfg.public_url);
    if let Err(e) = state.mailer.send(Email { to: to.to_owned(), subject: subject.into(), text }).await {
        tracing::error!(error = e, purpose, "failed to send email");
    }
    Ok(())
}

// ---------------------------------------------------------------- register
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RegisterReq {
    full_name: String,
    email: String,
    password: String,
}

/// Always answers 202 with the same body, whether or not the email already exists (no account enumeration).
pub async fn register(State(s): State<AppState>, ClientIp(ip): ClientIp, ApiJson(req): ApiJson<RegisterReq>) -> Result<Response, ApiError> {
    ratelimit::hit(&s.redis, &format!("register:ip:{ip}"), 10, 3600).await?;
    let email = validate::email(&req.email)?;
    let name = validate::person_name(&req.full_name)?;
    validate::password(&req.password, Some(&email))?;
    // Hash BEFORE touching the database so new and existing emails take comparable time.
    let pw = req.password.clone();
    let hash = tokio::task::spawn_blocking(move || security::hash_password(&pw)).await.map_err(ApiError::internal)??;

    let mut tx = begin_tenant_tx(&s.pool, TenantContext::default()).await?;
    let created: Result<Uuid, sqlx::Error> = sqlx::query_scalar("select app.register_user($1::citext, $2, $3)").bind(&email).bind(&name).bind(&hash).fetch_one(&mut *tx).await;
    match created {
        Ok(id) => {
            tx.commit().await?;
            issue_token_and_mail(&s, id, &email, "verify_email").await?;
        }
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23505") => {
            drop(tx);
            let notice = Email { to: email.clone(), subject: "You already have a PERMTPA ONE account".into(), text: format!("Someone tried to register with this email address. If it was you, log in or reset your password at {}/login.\nIf it was not you, no action is needed.", s.cfg.public_url) };
            if let Err(e) = s.mailer.send(notice).await {
                tracing::error!(error = e, "failed to send notice email");
            }
        }
        Err(e) => return Err(e.into()),
    }
    Ok(accepted())
}

// ---------------------------------------------------------------- email verification
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenReq {
    token: String,
}

pub async fn verify_email(State(s): State<AppState>, ClientIp(ip): ClientIp, ApiJson(req): ApiJson<TokenReq>) -> Result<Response, ApiError> {
    ratelimit::hit(&s.redis, &format!("verify:ip:{ip}"), 30, 3600).await?;
    let hash = security::hash_token(req.token.trim());
    let mut tx = begin_tenant_tx(&s.pool, TenantContext::default()).await?;
    let user: Option<Uuid> = sqlx::query_scalar("select app.verify_email($1)").bind(&hash[..]).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    match user {
        Some(_) => Ok(Json(json!({ "ok": true })).into_response()),
        None => Err(ApiError::bad("invalid_token", "This link is invalid or has expired.")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmailReq {
    email: String,
}

async fn lookup(s: &AppState, email: &str) -> Result<Option<(Uuid, bool, String)>, ApiError> {
    Ok(sqlx::query_as("select id, email_verified_at is not null, status from app.find_user_for_login($1::citext)").bind(email).fetch_optional(&s.pool).await?)
}

pub async fn resend_verification(State(s): State<AppState>, ClientIp(ip): ClientIp, ApiJson(req): ApiJson<EmailReq>) -> Result<Response, ApiError> {
    ratelimit::hit(&s.redis, &format!("resend:ip:{ip}"), 10, 3600).await?;
    let Ok(email) = validate::email(&req.email) else { return Ok(accepted()) };
    ratelimit::hit(&s.redis, &format!("resend:email:{}", email_key(&email)), 3, 3600).await?;
    if let Some((id, verified, status)) = lookup(&s, &email).await? {
        if !verified && status == "active" {
            issue_token_and_mail(&s, id, &email, "verify_email").await?;
        }
    }
    Ok(accepted())
}

// ---------------------------------------------------------------- login
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LoginReq {
    email: String,
    password: String,
    mfa_code: Option<String>,
}

pub async fn login(State(s): State<AppState>, ClientIp(ip): ClientIp, headers: HeaderMap, ApiJson(req): ApiJson<LoginReq>) -> Result<Response, ApiError> {
    ratelimit::hit(&s.redis, &format!("login:ip:{ip}"), 30, 60).await?;
    let email = validate::email(&req.email).map_err(|_| invalid_credentials())?;
    if req.password.is_empty() || req.password.chars().count() > 128 {
        return Err(invalid_credentials());
    }
    ratelimit::hit(&s.redis, &format!("login:email:{}", email_key(&email)), 10, 900).await?;

    type Row = (Uuid, String, String, bool, bool, bool);
    let user: Option<Row> = sqlx::query_as(
        "select id, password_hash, status, email_verified_at is not null, (locked_until is not null and locked_until > now()), mfa_enabled from app.find_user_for_login($1::citext)",
    )
    .bind(&email)
    .fetch_optional(&s.pool)
    .await?;

    // Always verify against SOME hash so response time does not reveal whether the account exists.
    let (hash, known) = match &user {
        Some(u) => (u.1.clone(), true),
        None => ((*s.dummy_hash).clone(), false),
    };
    let pw = req.password.clone();
    let pw_ok = tokio::task::spawn_blocking(move || security::verify_password(&hash, &pw)).await.map_err(ApiError::internal)?;
    let ip_text = ip.to_string();

    let Some((user_id, _, status, verified, locked, mfa_enabled)) = user.filter(|_| known) else { return Err(invalid_credentials()) };
    if locked || status != "active" {
        return Err(invalid_credentials()); // same answer: do not reveal lock/suspension state
    }
    if !pw_ok {
        sqlx::query("select app.record_login_failure($1, $2::text::inet)").bind(user_id).bind(&ip_text).execute(&s.pool).await?;
        return Err(invalid_credentials());
    }
    if !verified {
        return Err(ApiError::Forbidden { code: "email_not_verified" });
    }
    if mfa_enabled {
        let Some(code) = req.mfa_code.as_deref().filter(|c| !c.trim().is_empty()) else { return Err(ApiError::Unauthorized { code: "mfa_required" }) };
        let mut tx = begin_tenant_tx(&s.pool, TenantContext { user_id: Some(user_id), org_id: None }).await?;
        let ok = mfa::verify_second_factor(&mut tx, &s.cfg.mfa_key, user_id, code).await?;
        tx.commit().await?;
        if !ok {
            sqlx::query("select app.record_login_failure($1, $2::text::inet)").bind(user_id).bind(&ip_text).execute(&s.pool).await?;
            return Err(ApiError::Unauthorized { code: "invalid_mfa_code" });
        }
    }

    // Session fixation defence: a brand-new token on every login; any presented session is revoked.
    if let Some(old) = session::resolve(&s, &headers).await? {
        session::revoke(&s, &old, old.session_id).await?;
    }
    sqlx::query("select app.record_login_success($1, $2::text::inet)").bind(user_id).bind(&ip_text).execute(&s.pool).await?;
    let token = security::new_token()?;
    let token_hash = security::hash_token(&token);
    let mut tx = begin_tenant_tx(&s.pool, TenantContext { user_id: Some(user_id), org_id: None }).await?;
    sqlx::query("insert into sessions (user_id, token_hash, expires_at, user_agent, ip) values ($1, $2, now() + interval '30 days', $3, $4::text::inet)")
        .bind(user_id)
        .bind(&token_hash[..])
        .bind(session::user_agent(&headers))
        .bind(&ip_text)
        .execute(&mut *tx)
        .await?;
    let profile = profile_json(&mut tx, user_id).await?;
    tx.commit().await?;

    let mut res = Json(json!({ "user": profile, "csrfToken": security::csrf_token(&s.cfg.csrf_key, &token_hash) })).into_response();
    res.headers_mut().append(header::SET_COOKIE, session::set_cookie(&s.cfg, &token).parse().map_err(ApiError::internal)?);
    Ok(res)
}

async fn profile_json(tx: &mut sqlx::PgConnection, user_id: Uuid) -> Result<Value, ApiError> {
    let (email, name, verified, mfa): (String, String, bool, bool) =
        sqlx::query_as("select email::text, full_name, email_verified_at is not null, mfa_enabled from users where id = $1").bind(user_id).fetch_one(&mut *tx).await?;
    Ok(json!({ "id": user_id, "email": email, "fullName": name, "emailVerified": verified, "mfaEnabled": mfa }))
}

// ---------------------------------------------------------------- logout / csrf / me
pub async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    if let Some(sess) = session::resolve(&s, &headers).await? {
        session::revoke(&s, &sess, sess.session_id).await?;
    }
    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut().append(header::SET_COOKIE, session::clear_cookie(&s.cfg).parse().map_err(ApiError::internal)?);
    Ok(res)
}

pub async fn csrf(State(s): State<AppState>, sess: AuthSession) -> Json<Value> {
    Json(json!({ "csrfToken": sess.csrf(&s.cfg) }))
}

pub async fn me(State(s): State<AppState>, sess: AuthSession) -> Result<Json<Value>, ApiError> {
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let user = profile_json(&mut tx, sess.user_id).await?;
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "select o.id, o.name, o.slug::text, m.role::text from memberships m join organizations o on o.id = m.org_id where m.user_id = $1 and m.status = 'active' order by o.name",
    )
    .bind(sess.user_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.rollback().await?;
    let memberships: Vec<Value> = rows.into_iter().map(|(id, name, slug, role)| json!({ "orgId": id, "name": name, "slug": slug, "role": role })).collect();
    Ok(Json(json!({ "user": user, "memberships": memberships })))
}

// ---------------------------------------------------------------- password reset
pub async fn forgot_password(State(s): State<AppState>, ClientIp(ip): ClientIp, ApiJson(req): ApiJson<EmailReq>) -> Result<Response, ApiError> {
    ratelimit::hit(&s.redis, &format!("forgot:ip:{ip}"), 10, 3600).await?;
    let Ok(email) = validate::email(&req.email) else { return Ok(accepted()) };
    ratelimit::hit(&s.redis, &format!("forgot:email:{}", email_key(&email)), 3, 3600).await?;
    if let Some((id, _, status)) = lookup(&s, &email).await? {
        if status == "active" {
            issue_token_and_mail(&s, id, &email, "reset_password").await?;
        }
    }
    Ok(accepted())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ResetReq {
    token: String,
    new_password: String,
}

pub async fn reset_password(State(s): State<AppState>, ClientIp(ip): ClientIp, ApiJson(req): ApiJson<ResetReq>) -> Result<Response, ApiError> {
    ratelimit::hit(&s.redis, &format!("reset:ip:{ip}"), 20, 3600).await?;
    validate::password(&req.new_password, None)?;
    let pw = req.new_password.clone();
    let hash = tokio::task::spawn_blocking(move || security::hash_password(&pw)).await.map_err(ApiError::internal)??;
    let token_hash = security::hash_token(req.token.trim());
    let mut tx = begin_tenant_tx(&s.pool, TenantContext::default()).await?;
    let user: Option<Uuid> = sqlx::query_scalar("select app.reset_password($1, $2)").bind(&token_hash[..]).bind(&hash).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    let Some(_) = user else { return Err(ApiError::bad("invalid_token", "This link is invalid or has expired.")) };
    let mut res = Json(json!({ "ok": true })).into_response();
    res.headers_mut().append(header::SET_COOKIE, session::clear_cookie(&s.cfg).parse().map_err(ApiError::internal)?);
    Ok(res)
}

// ---------------------------------------------------------------- sessions
pub async fn list_sessions(State(s): State<AppState>, sess: AuthSession) -> Result<Json<Value>, ApiError> {
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let rows: Vec<(Uuid, String, String, Option<String>, Option<String>, bool)> = sqlx::query_as(
        "select id, created_at::text, last_seen_at::text, user_agent, host(ip), id = $1 from sessions where revoked_at is null and expires_at > now() order by last_seen_at desc",
    )
    .bind(sess.session_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.rollback().await?;
    let items: Vec<Value> = rows.into_iter().map(|(id, created, seen, ua, ip, current)| json!({ "id": id, "createdAt": created, "lastSeenAt": seen, "userAgent": ua, "ip": ip, "current": current })).collect();
    Ok(Json(json!({ "sessions": items })))
}

pub async fn revoke_session(State(s): State<AppState>, sess: AuthSession, Path(id): Path<Uuid>) -> Result<StatusCode, ApiError> {
    // RLS limits the UPDATE to the caller's own sessions, so another user's id simply matches nothing.
    if session::revoke(&s, &sess, id).await? { Ok(StatusCode::NO_CONTENT) } else { Err(ApiError::NotFound) }
}

pub async fn revoke_other_sessions(State(s): State<AppState>, sess: AuthSession) -> Result<Json<Value>, ApiError> {
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let n = sqlx::query("update sessions set revoked_at = now() where id <> $1 and revoked_at is null").bind(sess.session_id).execute(&mut *tx).await?.rows_affected();
    tx.commit().await?;
    Ok(Json(json!({ "revoked": n })))
}
