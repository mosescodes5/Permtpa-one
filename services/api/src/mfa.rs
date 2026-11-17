use crate::{
    app::AppState,
    db::{TenantContext, begin_tenant_tx},
    error::ApiError,
    ratelimit, security,
    session::{ApiJson, AuthSession},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Verifies a TOTP code (with replay protection) or burns a single-use recovery code.
/// Runs inside a transaction whose context is `user_id`, so RLS confines it to that user's rows.
pub async fn verify_second_factor(
    tx: &mut sqlx::PgConnection,
    key: &[u8; 32],
    user_id: Uuid,
    code: &str,
) -> Result<bool, ApiError> {
    let code = code.trim();
    if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
        let row: Option<(Vec<u8>, i64)> = sqlx::query_as("select secret_enc, last_used_step from user_mfa where user_id = $1 and confirmed_at is not null").bind(user_id).fetch_optional(&mut *tx).await?;
        let Some((enc, last)) = row else {
            return Ok(false);
        };
        let secret = security::decrypt(key, &enc)?;
        let Some(step) = security::verify_totp(&secret, code, now_unix(), last) else {
            return Ok(false);
        };
        // Conditional update makes replay protection race-safe: only one request can advance the step.
        let n = sqlx::query(
            "update user_mfa set last_used_step = $1 where user_id = $2 and last_used_step < $1",
        )
        .bind(step as i64)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        return Ok(n == 1);
    }
    let hash = security::hash_token(&security::normalize_recovery_code(code));
    let n = sqlx::query("update mfa_recovery_codes set used_at = now() where user_id = $1 and code_hash = $2 and used_at is null").bind(user_id).bind(&hash[..]).execute(&mut *tx).await?.rows_affected();
    Ok(n == 1)
}

pub async fn setup(State(s): State<AppState>, sess: AuthSession) -> Result<Json<Value>, ApiError> {
    ratelimit::hit(&s.redis, &format!("mfa:{}", sess.user_id), 20, 900).await?;
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let confirmed: Option<bool> =
        sqlx::query_scalar("select confirmed_at is not null from user_mfa where user_id = $1")
            .bind(sess.user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if confirmed == Some(true) {
        return Err(ApiError::Conflict {
            code: "mfa_already_enabled",
        });
    }
    let email: String = sqlx::query_scalar("select email::text from users where id = $1")
        .bind(sess.user_id)
        .fetch_one(&mut *tx)
        .await?;
    let secret = security::random_bytes::<20>()?;
    let enc = security::encrypt(&s.cfg.mfa_key, &secret)?;
    sqlx::query("delete from user_mfa where user_id = $1")
        .bind(sess.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("insert into user_mfa (user_id, secret_enc) values ($1, $2)")
        .bind(sess.user_id)
        .bind(&enc)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let b32 = security::base32(&secret);
    let uri = format!(
        "otpauth://totp/PERMTPA%20ONE:{}?secret={b32}&issuer=PERMTPA%20ONE&algorithm=SHA1&digits=6&period=30",
        urlencoding::encode(&email)
    );
    Ok(Json(json!({ "secret": b32, "otpauthUri": uri })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeReq {
    code: String,
}

pub async fn enable(
    State(s): State<AppState>,
    sess: AuthSession,
    ApiJson(req): ApiJson<CodeReq>,
) -> Result<Json<Value>, ApiError> {
    ratelimit::hit(&s.redis, &format!("mfa:{}", sess.user_id), 20, 900).await?;
    let mut tx = begin_tenant_tx(&s.pool, sess.ctx()).await?;
    let row: Option<(Vec<u8>, i64)> = sqlx::query_as("select secret_enc, last_used_step from user_mfa where user_id = $1 and confirmed_at is null").bind(sess.user_id).fetch_optional(&mut *tx).await?;
    let Some((enc, last)) = row else {
        return Err(ApiError::bad("mfa_not_started", "Start MFA setup first."));
    };
    let secret = security::decrypt(&s.cfg.mfa_key, &enc)?;
    let Some(step) = security::verify_totp(&secret, req.code.trim(), now_unix(), last) else {
        return Err(ApiError::bad(
            "invalid_mfa_code",
            "That code is not correct.",
        ));
    };
    sqlx::query("update user_mfa set confirmed_at = now(), last_used_step = $1 where user_id = $2")
        .bind(step as i64)
        .bind(sess.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("delete from mfa_recovery_codes where user_id = $1")
        .bind(sess.user_id)
        .execute(&mut *tx)
        .await?;
    let mut codes = Vec::with_capacity(10);
    for _ in 0..10 {
        let c = security::new_recovery_code()?;
        sqlx::query("insert into mfa_recovery_codes (user_id, code_hash) values ($1, $2)")
            .bind(sess.user_id)
            .bind(&security::hash_token(&c)[..])
            .execute(&mut *tx)
            .await?;
        codes.push(c);
    }
    sqlx::query("select app.set_mfa_enabled(true)")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    // Recovery codes are shown exactly once; only their hashes are stored.
    Ok(Json(json!({ "recoveryCodes": codes })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisableReq {
    password: String,
    code: String,
}

pub async fn disable(
    State(s): State<AppState>,
    sess: AuthSession,
    ApiJson(req): ApiJson<DisableReq>,
) -> Result<Json<Value>, ApiError> {
    ratelimit::hit(&s.redis, &format!("mfa:{}", sess.user_id), 20, 900).await?;
    let mut tx = begin_tenant_tx(
        &s.pool,
        TenantContext {
            user_id: Some(sess.user_id),
            org_id: None,
        },
    )
    .await?;
    let email: String = sqlx::query_scalar("select email::text from users where id = $1")
        .bind(sess.user_id)
        .fetch_one(&mut *tx)
        .await?;
    let hash: Option<String> =
        sqlx::query_scalar("select password_hash from app.find_user_for_login($1::citext)")
            .bind(&email)
            .fetch_optional(&mut *tx)
            .await?;
    let pw = req.password.clone();
    let pw_ok = match hash {
        Some(h) => tokio::task::spawn_blocking(move || security::verify_password(&h, &pw))
            .await
            .map_err(ApiError::internal)?,
        None => false,
    };
    if !pw_ok || !verify_second_factor(&mut tx, &s.cfg.mfa_key, sess.user_id, &req.code).await? {
        return Err(ApiError::Unauthorized {
            code: "invalid_credentials",
        });
    }
    sqlx::query("delete from mfa_recovery_codes where user_id = $1")
        .bind(sess.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("delete from user_mfa where user_id = $1")
        .bind(sess.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("select app.set_mfa_enabled(false)")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}
