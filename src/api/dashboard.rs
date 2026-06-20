use axum::{
    extract::State,
    response::Json,
    routing::get,
    Router,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Serialize;
use crate::server::app::AppState;
use crate::types::openai::ModelInfo;

#[derive(Debug, Serialize)]
pub struct DashboardData {
    pub providers: Vec<ProviderStatus>,
    pub metrics: MetricsData,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub provider_type: String,
    pub model_count: usize,
    pub color: String,
}

#[derive(Debug, Serialize)]
pub struct MetricsData {
    pub total_providers: usize,
    pub total_models: usize,
    pub built_in_free: bool,
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/dashboard", get(handle_dashboard))
}

async fn handle_dashboard(
    State(state): State<Arc<AppState>>,
) -> Json<DashboardData> {
    let mut providers = Vec::new();
    let mut total_models = 0;

    for provider in state.registry.all() {
        let cnt = provider.models().len();
        total_models += cnt;
        let is_free = provider.provider_type() == "opencode_free" || provider.provider_type() == "mimo_free";
        providers.push(ProviderStatus {
            name: provider.name().to_string(),
            provider_type: provider.provider_type().to_string(),
            model_count: cnt,
            color: if is_free { "#2da44e".into() } else { "#58a6ff".into() },
        });
    }

    providers.sort_by(|a, b| a.name.cmp(&b.name));

    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let models: Vec<ModelInfo> = state.registry.all().flat_map(|p| {
        let name = p.name().to_string();
        p.models().iter().map(move |m| ModelInfo {
            id: m.clone(),
            object: "model".into(),
            created: ts,
            owned_by: name.clone(),
        }).collect::<Vec<_>>()
    }).collect();

    Json(DashboardData {
        providers,
        metrics: MetricsData {
            total_providers: state.registry.all().count(),
            total_models,
            built_in_free: true,
        },
        models,
    })
}
