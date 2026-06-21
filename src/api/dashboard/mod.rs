// ─── Dashboard API ───────────────────────────────────────────────
// Split by resource domain: metrics, providers, routes, api_keys, settings.

pub mod types;
pub mod helpers;
pub mod providers;
pub mod routes;
pub mod api_keys;
pub mod settings;
pub mod metrics;

use std::sync::Arc;
use axum::Router;
use crate::server::app::AppState;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .merge(metrics::routes())
        .merge(providers::routes())
        .merge(routes::routes())
        .merge(api_keys::routes())
        .merge(settings::routes())
}
