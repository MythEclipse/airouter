use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json},
};
use arc_swap::ArcSwap;
use crate::auth::extract_bearer_token;
use crate::config::settings::RateLimitConfig;
use crate::server::app::AppState;

/// Redis-backed rate limiter using fixed window (INCR + EXPIRE)
pub struct RateLimitState {
    pub config: ArcSwap<RateLimitConfig>,
}

// Manual clone
impl Clone for RateLimitState {
    fn clone(&self) -> Self {
        Self { config: ArcSwap::from(self.config.load_full()) }
    }
}

impl RateLimitState {
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            config: ArcSwap::new(Arc::new(config.clone())),
        }
    }

    pub fn from_config(config: &RateLimitConfig) -> Self {
        Self::new(config)
    }

    pub async fn check_rate_limit(
        &self,
        redis: &redis::aio::ConnectionManager,
        key_hash: &str,
    ) -> Result<bool, String> {
        let config = self.config.load();
        if !config.enabled {
            return Ok(true);
        }
        let window = chrono::Utc::now().timestamp() / 60;
        let redis_key = format!("rate_limit:{}:{}", key_hash, window);

        // INCR + EXPIRE via command pipeline
        let mut conn = redis.clone();
        let current: i64 = redis::cmd("INCR")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("Redis error: {}", e))?;

        if current == 1 {
            let mut conn2 = redis.clone();
            let _: Result<(), _> = redis::cmd("EXPIRE")
                .arg(&redis_key)
                .arg(60i64)
                .query_async(&mut conn2)
                .await;
        }

        Ok(current <= config.requests_per_minute as i64)
    }
}

/// Rate limit middleware
pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

    let config = state.rate_limiter.config.load();
    if !config.enabled {
        drop(config);
        return Ok(next.run(req).await);
    }
    drop(config);

    let key = extract_bearer_token(req.headers()).unwrap_or_default();
    let key_hash = crate::auth::sha2_hex(&key);

    match state.rate_limiter.check_rate_limit(&state.redis, &key_hash).await {
        Ok(true) => Ok(next.run(req).await),
        _ => {
            let err = serde_json::json!({
                "error": {
                    "message": "Rate limit exceeded. Try again later.",
                    "type": "rate_limit_error",
                    "param": null,
                    "code": "rate_limit_exceeded"
                }
            });
            Err((StatusCode::TOO_MANY_REQUESTS, Json(err)))
        }
    }
}
