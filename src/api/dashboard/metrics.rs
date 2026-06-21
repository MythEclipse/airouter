use axum::{
    extract::State,
    response::Json,
    routing::get,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::server::app::AppState;
use crate::types::openai::ModelInfo;
use super::types::*;

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/dashboard", get(handle_dashboard))
        .route("/api/metrics", get(handle_provider_metrics))
}

async fn handle_dashboard(
    State(state): State<Arc<AppState>>,
) -> Json<DashboardData> {
    let mut providers = Vec::new();
    let mut total_models = 0;
    let mut provider_names = Vec::new();

    {
        let registry = state.registry.load();
        for provider in registry.all() {
            let cnt = provider.models().len();
            total_models += cnt;
            let ptype = provider.provider_type().to_string();
            let cat = crate::provider::category_for_type(&ptype)
                .unwrap_or(crate::provider::ProviderCategory::ApiKey);
            provider_names.push(provider.name().to_string());
            providers.push(ProviderStatus {
                name: provider.name().to_string(),
                provider_type: ptype.clone(),
                category: crate::provider::category_to_str(cat).to_string(),
                model_count: cnt,
                color: cat.color().into(),
                request_count: 0, error_count: 0, avg_latency_ms: 0.0, healthy: true,
            });
        }
    }

    let provider_metrics = state.tracker.provider_metrics(&state.redis, &provider_names).await;
    let pmap: std::collections::HashMap<_, _> = provider_metrics.into_iter()
        .map(|pm| (pm.name.clone(), pm)).collect();

    for p in &mut providers {
        if let Some(pm) = pmap.get(&p.name) {
            p.request_count = pm.request_count;
            p.error_count = pm.error_count;
            p.avg_latency_ms = pm.avg_latency_ms;
            p.healthy = pm.error_count == 0
                || (pm.request_count > 0 && (pm.error_count as f64) / (pm.request_count as f64) < 0.5);
        }
    }
    providers.sort_by(|a, b| a.name.cmp(&b.name));

    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let models: Vec<ModelInfo> = {
        let registry = state.registry.load();
        registry.all().flat_map(|p| {
            let name = p.name().to_string();
            p.models().iter().map(move |m| ModelInfo {
                id: m.clone(), object: "model".into(), created: ts, owned_by: name.clone(),
            }).collect::<Vec<_>>()
        }).collect()
    };

    let gm = state.tracker.global_metrics(&state.redis).await;
    let total = gm.total_requests;
    let errors = gm.total_errors;
    let registry_count = state.registry.load().all().count();

    Json(DashboardData {
        providers,
        metrics: MetricsData { total_providers: registry_count, total_models, built_in_free: true },
        models,
        live_metrics: LiveMetrics {
            total_requests: total, total_errors: errors,
            avg_latency_ms: gm.avg_latency_ms,
            error_rate: if total > 0 { errors as f64 / total as f64 } else { 0.0 },
            uptime_seconds: ts,
        },
    })
}

async fn handle_provider_metrics(
    State(state): State<Arc<AppState>>,
) -> Json<ProviderMetricsList> {
    let provider_names: Vec<String> = {
        let r = state.registry.load();
        r.all().map(|p| p.name().to_string()).collect()
    };
    let providers = state.tracker.provider_metrics(&state.redis, &provider_names).await;
    let global = state.tracker.global_metrics(&state.redis).await;
    Json(ProviderMetricsList { providers, global })
}
