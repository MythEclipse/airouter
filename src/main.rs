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

use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .json()
        .init();

    tracing::info!("Starting AIRouter...");

    let config_path = std::env::var("AIROUTER_CONFIG").unwrap_or_else(|_| "config.yaml".into());
    let settings = config::settings::Settings::load(&config_path)?;
    let settings = Arc::new(settings);

    tracing::info!(providers = %settings.providers.len(), routes = %settings.routes.len(), "Configuration loaded");
    tracing::info!(free_providers = %settings.providers.iter().filter(|p| p.provider_type == "opencode_free" || p.provider_type == "mimo_free").count(), "Free providers available");

    let registry = Arc::new(provider::ProviderRegistry::from_config(&settings.providers));
    tracing::info!(provider_count = %registry.all().count(), "Provider registry initialized");

    let rate_limiter = rate_limit::RateLimitState::from_config(&settings.rate_limit);

    let app_state = server::app::AppState {
        settings: settings.clone(),
        registry: registry.clone(),
        rate_limiter: rate_limiter.clone(),
    };

    let app = server::app::create_router(app_state, settings.clone(), registry.clone());

    let addr = format!("{}:{}", settings.server.host, settings.server.port);
    tracing::info!(addr = %addr, "Server listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
