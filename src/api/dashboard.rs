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
    pub live_metrics: LiveMetrics,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub name: String,
    pub provider_type: String,
    pub model_count: usize,
    pub color: String,
    pub request_count: u64,
    pub error_count: u64,
    pub avg_latency_ms: f64,
    pub healthy: bool,
}

#[derive(Debug, Serialize)]
pub struct MetricsData {
    pub total_providers: usize,
    pub total_models: usize,
    pub built_in_free: bool,
}

#[derive(Debug, Serialize)]
pub struct LiveMetrics {
    pub total_requests: u64,
    pub total_errors: u64,
    pub avg_latency_ms: f64,
    pub error_rate: f64,
    pub uptime_seconds: u64,
}

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/dashboard", get(handle_dashboard))
        .route("/api/metrics", get(handle_provider_metrics))
}

async fn handle_dashboard(
    State(state): State<Arc<AppState>>,
) -> Json<DashboardData> {
    let mut providers = Vec::new();
    let mut total_models = 0;

    // Get provider-level metrics from tracker
    let provider_metrics = state.tracker.provider_metrics();
    let provider_metrics_map: std::collections::HashMap<String, crate::tracker::ProviderMetrics> = provider_metrics
        .into_iter()
        .map(|pm| (pm.name.clone(), pm))
        .collect();

    for provider in state.registry.all() {
        let cnt = provider.models().len();
        total_models += cnt;
        let is_free = provider.provider_type() == "opencode_free" || provider.provider_type() == "mimo_free";

        let pm = provider_metrics_map.get(provider.name());
        let request_count = pm.map(|p| p.request_count).unwrap_or(0);
        let error_count = pm.map(|p| p.error_count).unwrap_or(0);
        let avg_latency = pm.map(|p| p.avg_latency_ms).unwrap_or(0.0);

        providers.push(ProviderStatus {
            name: provider.name().to_string(),
            provider_type: provider.provider_type().to_string(),
            model_count: cnt,
            color: if is_free { "#2da44e".into() } else { "#58a6ff".into() },
            request_count,
            error_count,
            avg_latency_ms: avg_latency,
            healthy: error_count == 0 || (request_count > 0 && (error_count as f64) / (request_count as f64) < 0.5),
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

    let gm = state.tracker.global_metrics();
    let total = gm.total_requests;
    let errors = gm.total_errors;

    Json(DashboardData {
        providers,
        metrics: MetricsData {
            total_providers: state.registry.all().count(),
            total_models,
            built_in_free: true,
        },
        models,
        live_metrics: LiveMetrics {
            total_requests: total,
            total_errors: errors,
            avg_latency_ms: gm.avg_latency_ms,
            error_rate: if total > 0 { errors as f64 / total as f64 } else { 0.0 },
            uptime_seconds: ts,
        },
    })
}

#[derive(Debug, Serialize)]
pub struct ProviderMetricsList {
    pub providers: Vec<crate::tracker::ProviderMetrics>,
    pub global: crate::tracker::GlobalMetrics,
}

async fn handle_provider_metrics(
    State(state): State<Arc<AppState>>,
) -> Json<ProviderMetricsList> {
    let providers = state.tracker.provider_metrics();
    let global = state.tracker.global_metrics();
    Json(ProviderMetricsList { providers, global })
}
