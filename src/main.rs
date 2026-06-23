mod config;
mod server;
mod api;
mod provider;
mod router;
mod transform;
mod auth;
mod oauth;
mod rate_limit;
mod streaming;
mod types;
pub mod tracker;
mod entities;
mod logging;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use arc_swap::ArcSwap;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::layer::Layer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // ── Initialize structured JSON logging with error dedup ──────────
    let dedup_state = Arc::new(Mutex::new(
        crate::logging::DedupState::new(Duration::from_secs(1)),
    ));
    let json_format = tracing_subscriber::fmt::format().json();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .event_format(crate::logging::DedupFormatEvent::new(
                    json_format,
                    dedup_state,
                ))
                .with_filter(
                    EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                ),
        )
        .init();

    tracing::info!("Starting AIRouter...");

    // ── Load .env for DATABASE_URL only ─────────────────────────────
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
    config::db::seed_password(&db).await?;
    tracing::info!("Database connected and initialized");

    // ── Connect to Redis ────────────────────────────────────────────
    let redis_client = redis::Client::open(redis_url)?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client.clone()).await?;
    let key_store = crate::auth::key_store::KeyStore::new(
        redis_conn.clone(),
        redis_client,
    ).await?;
    let _key_invalidation = key_store.spawn_invalidation_listener();
    let _key_periodic_sync = key_store.spawn_periodic_sync();
    tracing::info!("Redis connected");

    // ── Load config from DB (single source of truth) ────────────────
    let settings = Arc::new(ArcSwap::new(Arc::new(
        config::db::load_config_from_db(&db).await?
    )));
    tracing::info!(providers = %settings.load().providers.len(), routes = %settings.load().routes.len(), "Configuration loaded from database");

    // ── Build provider registry ─────────────────────────────────────
    let registry = Arc::new(ArcSwap::new(Arc::new(
        provider::ProviderRegistry::from_config(&settings.load().providers)
    )));
    tracing::info!(provider_count = %registry.load().all().count(), "Provider registry initialized");

    // ── JWT secret store (Postgres-backed, used for rotation) ─────────
    let jwt_secrets = crate::auth::jwt_secret_store::JwtSecretStore::new(db.clone()).await?;
    let _jwt_refresh_handle = jwt_secrets.spawn_refresh_task();
    tracing::info!("JWT secret store initialized");

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
        key_store: key_store.clone(),
        jwt_secrets: jwt_secrets.clone(),
        rate_limiter,
        balancer,
        engine,
        tracker: request_tracker,
        prometheus_handle,
    };

    let addr = format!("{}:{}", settings.load().server.host, settings.load().server.port);
    tracing::info!(addr = %addr, "Server listening");

    let app = server::app::create_router(
        app_state,
        settings.load_full(),
        registry.load_full(),
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

pub use server::app::AppState;
