mod config;
mod server;
mod api;
mod provider;
mod router;
mod transform;
mod auth;
mod rate_limit;
mod streaming;
mod types;
pub mod tracker;
mod entities;

use std::sync::Arc;
use std::collections::HashSet;
use arc_swap::ArcSwap;
use tracing_subscriber::EnvFilter;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .json()
        .init();

    tracing::info!("Starting AIRouter...");

    // ── Load .env ───────────────────────────────────────────────────
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in .env or environment");
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    // ── Connect to PostgreSQL ───────────────────────────────────────
    use sea_orm::Database;
    let db = Database::connect(&database_url).await?;
    config::db::run_migrations(&db).await?;
    config::db::seed_defaults(&db).await?;
    tracing::info!("Database connected and initialized");

    // ── Connect to Redis ────────────────────────────────────────────
    let redis_client = redis::Client::open(redis_url)?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    tracing::info!("Redis connected");

    // ── Load config from DB ─────────────────────────────────────────
    let settings_from_db = config::db::load_config_from_db(&db).await?;
    let settings = Arc::new(ArcSwap::new(Arc::new(settings_from_db)));
    tracing::info!(providers = %settings.load().providers.len(), routes = %settings.load().routes.len(), "Configuration loaded from database");

    // ── Build provider registry ─────────────────────────────────────
    let registry = Arc::new(ArcSwap::new(Arc::new(
        provider::ProviderRegistry::from_config(&settings.load().providers)
    )));
    tracing::info!(provider_count = %registry.load().all().count(), "Provider registry initialized");

    // ── Load key hashes for auth ────────────────────────────────────
    let key_hashes = Arc::new(ArcSwap::new(Arc::new(load_key_hashes(&db).await)));

    // ── Redis-backed state components ───────────────────────────────
    let rate_limiter = rate_limit::RateLimitState::from_config(&settings.load().rate_limit);
    let request_tracker = tracker::RequestTracker::new();

    // Initialize Prometheus metrics exporter
    let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .ok();

    let balancer = Arc::new(router::balancer::LoadBalancer::new(redis_conn.clone(), 30));
    let engine = Arc::new(router::core::RouteEngine::new(
        registry.clone(),
        settings.clone(),
        balancer.clone(),
        redis_conn.clone(),
    ));

    let app_state = server::app::AppState {
        db,
        redis: redis_conn,
        config: settings.clone(),
        registry: registry.clone(),
        key_hashes: key_hashes.clone(),
        rate_limiter,
        balancer: balancer.clone(),
        engine: engine.clone(),
        tracker: request_tracker.clone(),
        prometheus_handle,
    };

    let addr = format!("{}:{}", settings.load().server.host, settings.load().server.port);
    tracing::info!(addr = %addr, "Server listening");

    // Build router with Arc-wrapped Settings for backward compat
    let app = server::app::create_router(
        app_state,
        settings.load_full(),
        registry.load_full(),
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Load enabled API key hashes from database
async fn load_key_hashes(db: &sea_orm::DatabaseConnection) -> HashSet<String> {
    use crate::entities::api_key;
    use sea_orm::{EntityTrait, ColumnTrait, QueryFilter};
    api_key::Entity::find()
        .filter(api_key::Column::Enabled.eq(true))
        .all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.key_hash)
        .collect()
}

/// Re-export AppState for other modules
pub use server::app::AppState;
