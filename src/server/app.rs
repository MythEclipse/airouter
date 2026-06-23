use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{from_fn, from_fn_with_state},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use std::collections::HashSet;
use std::sync::Arc;
use arc_swap::ArcSwap;
use tower_http::cors::CorsLayer;
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

    let api_routes = crate::api::openai::routes(state.clone())
        .merge(crate::api::anthropic::routes(state.clone()))
        .merge(crate::api::dashboard::routes(state.clone()))
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
        .merge(api_routes)
        // Auth routes (login is public — handled in middleware)
        .merge(crate::api::auth::routes(state.clone()))
        // OAuth routes (public for authorization flow)
        .merge(crate::api::oauth::routes(state.clone()))
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
    pub key_hashes: Arc<ArcSwap<HashSet<String>>>,
    pub jwt_secret: Arc<ArcSwap<String>>,
    pub jwt_secrets: Arc<crate::auth::jwt_secret_store::JwtSecretStore>,
    pub password_hash: Arc<ArcSwap<String>>,
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

        // Load key hashes from DB (separate call so key hot-reload is not dependent on provider/route queries)
        self.reload_key_hashes().await?;

        tracing::info!("Configuration hot-reloaded from database");
        Ok(())
    }

    /// Reload only the key hashes from DB — fast path for API key CRUD
    pub async fn reload_key_hashes(&self) -> Result<(), sea_orm::DbErr> {
        use crate::entities::api_key;
        use sea_orm::{EntityTrait, ColumnTrait, QueryFilter};
        let key_rows = api_key::Entity::find()
            .filter(api_key::Column::Enabled.eq(true))
            .all(&self.db).await?;
        let hashes: HashSet<String> = key_rows.into_iter().map(|r| r.key_hash).collect();
        self.key_hashes.store(Arc::new(hashes));
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
