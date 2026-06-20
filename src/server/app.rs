use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{from_fn, from_fn_with_state},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_http::services::ServeDir;
use uuid::Uuid;
use crate::auth::middleware::auth_middleware;
use crate::provider;
use crate::router::balancer::LoadBalancer;
use crate::router::core::RouteEngine;

const FRONTEND_DIST: &str = "frontend-dist";

pub fn create_router(
    state: AppState,
    _settings: Arc<crate::config::settings::Settings>,
    _registry: Arc<provider::ProviderRegistry>,
) -> Router {
    let state = Arc::new(state);

    let api_routes = crate::api::openai::routes(state.clone())
        .merge(crate::api::anthropic::routes(state.clone()))
        .merge(crate::api::dashboard::routes(state.clone()))
        // Rate limiter middleware on API routes
        .route_layer(from_fn_with_state(
            state.clone(),
            crate::rate_limit::rate_limit_middleware,
        ))
        // Auth middleware on API routes
        .route_layer(from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/health", axum::routing::get(health_check))
        // Prometheus metrics
        .route("/metrics", get(handle_metrics))
        // Request ID middleware for ALL routes
        .layer(from_fn(request_id_middleware))
        // API routes with auth + rate limit
        .merge(api_routes)
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
    pub db: sea_orm::DatabaseConnection,
    pub settings: Arc<crate::config::settings::Settings>,
    pub registry: Arc<provider::ProviderRegistry>,
    pub rate_limiter: crate::rate_limit::RateLimitState,
    pub balancer: Arc<LoadBalancer>,
    pub engine: Arc<RouteEngine>,
    pub tracker: crate::tracker::RequestTracker,
    pub prometheus_handle: Option<metrics_exporter_prometheus::PrometheusHandle>,
}

async fn health_check() -> &'static str {
    "OK"
}

async fn handle_metrics(
    State(state): State<Arc<AppState>>,
) -> Response {
    if let Some(ref handle) = state.prometheus_handle {
        handle.render().into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "Prometheus not initialized").into_response()
    }
}

/// Add X-Request-Id to every request/response
async fn request_id_middleware(
    mut req: Request,
    next: axum::middleware::Next,
) -> Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    req.extensions_mut().insert(request_id.clone());

    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );
    resp
}
