use crate::error::ApiError;

/// Fixed-window counter in Redis. FAILS CLOSED: if Redis is down, authentication endpoints refuse service
/// rather than silently losing brute-force protection.
pub async fn hit(redis: &redis::Client, key: &str, limit: i64, window_secs: u64) -> Result<(), ApiError> {
    let mut conn = redis.get_multiplexed_async_connection().await.map_err(|e| {
        tracing::error!(error = %e, "rate limiter: redis unavailable");
        ApiError::Unavailable
    })?;
    let full = format!("rl:{key}");
    let (count, ttl): (i64, i64) = redis::pipe()
        .atomic()
        .cmd("INCR").arg(&full)
        .cmd("TTL").arg(&full)
        .query_async(&mut conn)
        .await
        .map_err(|e| { tracing::error!(error = %e, "rate limiter: redis error"); ApiError::Unavailable })?;
    if count == 1 || ttl < 0 {
        let _: () = redis::cmd("EXPIRE").arg(&full).arg(window_secs).query_async(&mut conn).await.map_err(|e| { tracing::error!(error = %e, "rate limiter: expire failed"); ApiError::Unavailable })?;
    }
    if count > limit {
        let retry_after = if ttl > 0 { ttl as u64 } else { window_secs };
        return Err(ApiError::TooManyRequests { retry_after });
    }
    Ok(())
}
