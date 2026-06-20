use std::num::NonZeroU32;
use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json},
};
use dashmap::DashMap;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use crate::auth::extract_bearer_token;
use crate::config::settings::RateLimitConfig;
use crate::server::app::AppState;

#[derive(Clone)]
pub struct RateLimitState {
    pub limiters: Arc<DashMap<String, Arc<DefaultDirectRateLimiter>>>,
    pub config: RateLimitConfig,
}

impl RateLimitState {
    pub fn from_config(config: &RateLimitConfig) -> Self {
        Self {
            limiters: Arc::new(DashMap::new()),
            config: config.clone(),
        }
    }

    pub fn get_limiter(&self, key: &str) -> Arc<DefaultDirectRateLimiter> {
        self.limiters
            .entry(key.to_string())
            .or_insert_with(|| {
                let rpm = NonZeroU32::new(self.config.requests_per_minute as u32)
                    .unwrap_or(NonZeroU32::new(60).unwrap());
                let burst = NonZeroU32::new(self.config.burst_size)
                    .unwrap_or(NonZeroU32::new(20).unwrap());
                let quota = Quota::per_minute(rpm).allow_burst(burst);
                Arc::new(RateLimiter::direct(quota))
            })
            .value()
            .clone()
    }
}

/// Rate limit middleware. The router uses Arc<AppState>, so accept that here.
pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    if req.uri().path() == "/health" || !state.settings.rate_limit.enabled {
        return Ok(next.run(req).await);
    }

    let key = extract_bearer_token(req.headers()).unwrap_or_default();
    let limiter = state.rate_limiter.get_limiter(&key);

    match limiter.check() {
        Ok(_) => Ok(next.run(req).await),
        Err(_) => {
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

/// Rate limit middleware that takes an optional RateLimitState directly (for use without Arc<AppState>).
/// Only used where middleware is applied differently.
pub async fn rate_limit_layer_fn(
    State(rate_limiter): State<crate::rate_limit::RateLimitState>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }

    let key = extract_bearer_token(req.headers()).unwrap_or_default();
    let limiter = rate_limiter.get_limiter(&key);

    match limiter.check() {
        Ok(_) => Ok(next.run(req).await),
        Err(_) => {
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
