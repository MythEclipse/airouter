use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{from_fn, from_fn_with_state},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use arc_swap::ArcSwap;
use crate::auth::key_store::KeyStore;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tower_http::services::{ServeDir, ServeFile};
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

    // ── Sub-routers with per-route-group request body size limits ──────
    // Body limit layers are applied to each sub-router BEFORE merging
    // into the main router, ensuring every route group gets its own
    // maximum payload size.
    let completions_routes = crate::api::openai::completions_routes(state.clone())
        .layer(RequestBodyLimitLayer::new(2_000_000)); // 2 MB — chat payloads

    let models_routes = crate::api::openai::models_routes(state.clone())
        .layer(RequestBodyLimitLayer::new(1_000)); // 1 KB — model listing only

    let anthropic_routes = crate::api::anthropic::routes(state.clone())
        .layer(RequestBodyLimitLayer::new(2_000_000)); // 2 MB — messages

    let dashboard_routes = crate::api::dashboard::routes(state.clone())
        .layer(RequestBodyLimitLayer::new(512_000)); // 512 KB — admin CRUD

    let auth_routes = crate::api::auth::routes(state.clone())
        .layer(RequestBodyLimitLayer::new(2_000)); // 2 KB — credentials only

    let oauth_routes = crate::api::oauth::routes(state.clone())
        .layer(RequestBodyLimitLayer::new(64_000)); // 64 KB — OAuth tokens

    // ── AI routes (protected by auth + rate_limit) ─────────────────────
    let ai_routes = completions_routes
        .merge(models_routes)
        .merge(anthropic_routes)
        .merge(dashboard_routes)
        .route_layer(from_fn_with_state(
            state.clone(),
            crate::rate_limit::rate_limit_middleware,
        ))
        .route_layer(from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/health", axum::routing::get(health_check))
        .route("/metrics", get(handle_metrics))
        .layer(from_fn(request_id_middleware))
        .merge(ai_routes)
        // Auth routes (login is public — handled in middleware)
        .merge(auth_routes)
        // OAuth routes (public for authorization flow)
        .merge(oauth_routes)
        .fallback_service(
            ServeDir::new(FRONTEND_DIST)
                .append_index_html_on_directories(true)
                .fallback(ServeFile::new(format!("{}/index.html", FRONTEND_DIST)))
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Clone)]
pub struct AppState {
    pub db: sea_orm::DatabaseConnection,
    pub redis: redis::aio::ConnectionManager,
    pub config: Arc<ArcSwap<crate::config::settings::Settings>>,
    pub registry: Arc<ArcSwap<provider::ProviderRegistry>>,
    // TODO(Task 11): remove after migration to KeyStore
    // pub key_hashes: Arc<ArcSwap<HashSet<String>>>,
    pub key_store: Arc<KeyStore>,
    pub jwt_secrets: Arc<crate::auth::jwt_secret_store::JwtSecretStore>,
    pub rate_limiter: crate::rate_limit::RateLimitState,
    pub balancer: Arc<LoadBalancer>,
    pub engine: Arc<RouteEngine>,
    pub tracker: crate::tracker::RequestTracker,
    pub prometheus_handle: Option<metrics_exporter_prometheus::PrometheusHandle>,
}

impl AppState {
    /// Hot-reload config from database
    pub async fn reload_config(&self) -> Result<(), sea_orm::DbErr> {
        let settings = crate::config::db::load_config_from_db(&self.db).await?;
        let registry = provider::ProviderRegistry::from_config(&settings.providers);

        self.config.store(Arc::new(settings));
        self.registry.store(Arc::new(registry));

        // Sync key hashes from Redis (source of truth moved to KeyStore)
        if let Err(e) = self.key_store.full_sync().await {
            tracing::warn!(error = %e, "KeyStore full sync during reload_config");
        }

        tracing::info!("Configuration hot-reloaded from database");
        Ok(())
    }

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
