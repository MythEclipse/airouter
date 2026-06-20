use axum::{Router, middleware::from_fn_with_state};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
use crate::auth::middleware::auth_middleware;
use crate::provider;

const FRONTEND_DIST: &str = "frontend-dist";

pub fn create_router(
    state: AppState,
    _settings: Arc<crate::config::settings::Settings>,
    _registry: Arc<provider::ProviderRegistry>,
) -> Router {
    let state = Arc::new(state);

    Router::new()
        .route("/health", axum::routing::get(health_check))
        // API routes — with per-route auth layer
        .merge(
            // Build API routes first
            crate::api::openai::routes(state.clone())
                .merge(crate::api::anthropic::routes(state.clone()))
                .merge(crate::api::dashboard::routes(state.clone()))
                // Auth ONLY applies to these explicit routes, not unmatched requests
                .route_layer(from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                ))
        )
        // Frontend static — no auth, catches everything else
        .fallback_service(
            ServeDir::new(FRONTEND_DIST).append_index_html_on_directories(true)
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<crate::config::settings::Settings>,
    pub registry: Arc<provider::ProviderRegistry>,
    pub rate_limiter: crate::rate_limit::RateLimitState,
}

async fn health_check() -> &'static str {
    "OK"
}
