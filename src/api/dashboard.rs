use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use std::sync::Arc;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use sea_orm::{EntityTrait, ColumnTrait, QueryFilter, ActiveModelTrait, Set, ModelTrait};
use crate::server::app::AppState;
use crate::types::openai::ModelInfo;
use crate::auth::sha2_hex;

// ─── Response types ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DashboardData {
    pub providers: Vec<ProviderStatus>,
    pub metrics: MetricsData,
    pub models: Vec<ModelInfo>,
    pub live_metrics: LiveMetrics,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub name: String, pub provider_type: String, pub model_count: usize,
    pub color: String, pub request_count: u64, pub error_count: u64,
    pub avg_latency_ms: f64, pub healthy: bool,
}

#[derive(Debug, Serialize)]
pub struct MetricsData {
    pub total_providers: usize, pub total_models: usize, pub built_in_free: bool,
}

#[derive(Debug, Serialize)]
pub struct LiveMetrics {
    pub total_requests: u64, pub total_errors: u64, pub avg_latency_ms: f64,
    pub error_rate: f64, pub uptime_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderResponse {
    pub id: String, pub name: String, pub provider_type: String,
    pub api_key: String, pub base_url: String,
    pub models: Vec<String>, pub capabilities: Vec<String>,
    pub enabled: bool, pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RouteResponse {
    pub id: String, pub model: String, pub strategy: String,
    pub provider: Option<String>, pub providers: Option<Vec<String>>,
    pub combo: serde_json::Value, pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyResponse {
    pub id: String, pub key_name: String, pub key_prefix: String,
    pub enabled: bool, pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub server: ServerSettingsResponse,
    pub rate_limit: RateLimitSettingsResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerSettingsResponse { pub host: String, pub port: i32 }

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitSettingsResponse {
    pub enabled: bool, pub requests_per_minute: i64, pub burst_size: i32,
}

// ─── Request types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String, pub provider_type: String,
    #[serde(default)] pub api_key: String,
    #[serde(default)] pub base_url: String,
    #[serde(default)] pub models: Vec<String>,
    #[serde(default)] pub capabilities: Vec<String>,
    #[serde(default)] pub extra_headers: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProviderRequest {
    pub name: Option<String>, pub provider_type: Option<String>,
    pub api_key: Option<String>, pub base_url: Option<String>,
    pub models: Option<Vec<String>>, pub capabilities: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRouteRequest {
    pub model: String, pub strategy: String,
    pub provider: Option<String>, pub providers: Option<Vec<String>>,
    pub combo: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRouteRequest {
    pub model: Option<String>, pub strategy: Option<String>,
    pub provider: Option<Option<String>>,
    pub providers: Option<Option<Vec<String>>>,
    pub combo: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub key_name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String, pub key_name: String,
    pub key_prefix: String, pub full_key: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub server: Option<ServerSettingsUpdate>,
    pub rate_limit: Option<RateLimitSettingsUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettingsUpdate { pub host: Option<String>, pub port: Option<i32> }

#[derive(Debug, Deserialize)]
pub struct RateLimitSettingsUpdate {
    pub enabled: Option<bool>, pub requests_per_minute: Option<i64>,
    pub burst_size: Option<i32>,
}

// ─── Routes ──────────────────────────────────────────────────────

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Dashboard / Monitor
        .route("/api/dashboard", get(handle_dashboard))
        .route("/api/metrics", get(handle_provider_metrics))
        // Providers CRUD
        .route("/api/dashboard/providers", get(list_providers).post(create_provider))
        .route("/api/dashboard/providers/:id", get(get_provider).put(update_provider).delete(delete_provider))
        // Routes CRUD
        .route("/api/dashboard/routes", get(list_routes).post(create_route))
        .route("/api/dashboard/routes/:id", get(get_route).put(update_route).delete(delete_route))
        // API Keys CRUD
        .route("/api/dashboard/api-keys", get(list_api_keys).post(create_api_key))
        .route("/api/dashboard/api-keys/:id", delete(delete_api_key))
        // Settings
        .route("/api/dashboard/settings", get(get_settings).put(update_settings))
}

// ─── Dashboard handlers ──────────────────────────────────────────

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
            let is_free = provider.provider_type() == "opencode_free" || provider.provider_type() == "mimo_free";
            provider_names.push(provider.name().to_string());
            providers.push(ProviderStatus {
                name: provider.name().to_string(),
                provider_type: provider.provider_type().to_string(),
                model_count: cnt,
                color: if is_free { "#2da44e".into() } else { "#58a6ff".into() },
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
            p.healthy = pm.error_count == 0 || (pm.request_count > 0 && (pm.error_count as f64) / (pm.request_count as f64) < 0.5);
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

#[derive(Debug, Serialize)]
pub struct ProviderMetricsList {
    pub providers: Vec<crate::tracker::ProviderMetrics>,
    pub global: crate::tracker::GlobalMetrics,
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

// ─── Providers CRUD ──────────────────────────────────────────────

async fn list_providers(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ProviderResponse>> {
    use crate::entities::provider;
    let rows = provider::Entity::find().all(&state.db).await.unwrap_or_default();
    Json(rows.into_iter().map(row_to_provider_response).collect())
}

async fn get_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProviderResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let row = provider::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Provider not found"))?;
    Ok(Json(row_to_provider_response(row)))
}

async fn create_provider(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<ProviderResponse>), (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let extra = if body.extra_headers.is_null() { serde_json::Value::Object(Default::default()) } else { body.extra_headers };

    let model = provider::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(body.name),
        provider_type: Set(body.provider_type),
        api_key: Set(body.api_key),
        base_url: Set(body.base_url),
        models: Set(body.models),
        extra_headers: Set(extra),
        capabilities: Set(body.capabilities),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    let row = model.insert(&state.db).await
        .map_err(|e| err_400(&format!("Insert failed: {}", e)))?;
    state.reload_config().await.ok();
    Ok((StatusCode::CREATED, Json(row_to_provider_response(row))))
}

async fn update_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let existing = provider::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Provider not found"))?;

    let mut model: provider::ActiveModel = existing.into();
    if let Some(v) = body.name { model.name = Set(v); }
    if let Some(v) = body.provider_type { model.provider_type = Set(v); }
    if let Some(v) = body.api_key { model.api_key = Set(v); }
    if let Some(v) = body.base_url { model.base_url = Set(v); }
    if let Some(v) = body.models { model.models = Set(v); }
    if let Some(v) = body.capabilities { model.capabilities = Set(v); }
    if let Some(v) = body.enabled { model.enabled = Set(v); }
    model.updated_at = Set(chrono::Utc::now());

    let row = model.update(&state.db).await
        .map_err(|e| err_400(&format!("Update failed: {}", e)))?;
    state.reload_config().await.ok();
    Ok(Json(row_to_provider_response(row)))
}

async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let existing = provider::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Provider not found"))?;
    existing.delete(&state.db).await.map_err(|_| err_500("Delete failed"))?;
    state.reload_config().await.ok();
    Ok(StatusCode::NO_CONTENT)
}

fn row_to_provider_response(row: crate::entities::provider::Model) -> ProviderResponse {
    ProviderResponse {
        id: row.id.to_string(),
        name: row.name,
        provider_type: row.provider_type,
        api_key: "[REDACTED]".into(),
        base_url: row.base_url,
        models: row.models,
        capabilities: row.capabilities,
        enabled: row.enabled,
        created_at: row.created_at.to_rfc3339(),
    }
}

// ─── Routes CRUD ─────────────────────────────────────────────────

async fn list_routes(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<RouteResponse>> {
    use crate::entities::route;
    let rows = route::Entity::find().all(&state.db).await.unwrap_or_default();
    Json(rows.into_iter().map(|r| RouteResponse {
        id: r.id.to_string(), model: r.model, strategy: r.strategy,
        provider: r.provider, providers: r.providers,
        combo: r.combo, enabled: r.enabled,
    }).collect())
}

async fn get_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RouteResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;
    let row = route::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Route not found"))?;
    Ok(Json(RouteResponse {
        id: row.id.to_string(), model: row.model, strategy: row.strategy,
        provider: row.provider, providers: row.providers,
        combo: row.combo, enabled: row.enabled,
    }))
}

async fn create_route(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRouteRequest>,
) -> Result<(StatusCode, Json<RouteResponse>), (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;
    let combo = body.combo.unwrap_or(serde_json::Value::Null);

    let model = route::ActiveModel {
        id: Set(Uuid::new_v4()),
        model: Set(body.model),
        strategy: Set(body.strategy),
        provider: Set(body.provider),
        providers: Set(body.providers),
        combo: Set(combo),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    let row = model.insert(&state.db).await
        .map_err(|e| err_400(&format!("Insert failed: {}", e)))?;
    state.reload_config().await.ok();
    Ok((StatusCode::CREATED, Json(RouteResponse {
        id: row.id.to_string(), model: row.model, strategy: row.strategy,
        provider: row.provider, providers: row.providers,
        combo: row.combo, enabled: row.enabled,
    })))
}

async fn update_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRouteRequest>,
) -> Result<Json<RouteResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;
    let existing = route::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Route not found"))?;

    let mut model: route::ActiveModel = existing.into();
    if let Some(v) = body.model { model.model = Set(v); }
    if let Some(v) = body.strategy { model.strategy = Set(v); }
    if let Some(v) = body.provider { model.provider = Set(v); }
    if let Some(v) = body.providers { model.providers = Set(v); }
    if let Some(v) = body.combo { model.combo = Set(v); }
    if let Some(v) = body.enabled { model.enabled = Set(v); }
    model.updated_at = Set(chrono::Utc::now());

    let row = model.update(&state.db).await
        .map_err(|e| err_400(&format!("Update failed: {}", e)))?;
    state.reload_config().await.ok();
    Ok(Json(RouteResponse {
        id: row.id.to_string(), model: row.model, strategy: row.strategy,
        provider: row.provider, providers: row.providers,
        combo: row.combo, enabled: row.enabled,
    }))
}

async fn delete_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;
    let existing = route::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Route not found"))?;
    existing.delete(&state.db).await.map_err(|_| err_500("Delete failed"))?;
    state.reload_config().await.ok();
    Ok(StatusCode::NO_CONTENT)
}

// ─── API Keys CRUD ───────────────────────────────────────────────

async fn list_api_keys(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ApiKeyResponse>> {
    use crate::entities::api_key;
    let rows = api_key::Entity::find().all(&state.db).await.unwrap_or_default();
    Json(rows.into_iter().map(|r| ApiKeyResponse {
        id: r.id.to_string(), key_name: r.key_name,
        key_prefix: r.key_prefix, enabled: r.enabled,
        created_at: r.created_at.to_rfc3339(),
    }).collect())
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::api_key;
    let full_key = format!("sk-{}", Uuid::new_v4().to_string().replace("-", ""));
    let prefix = full_key[..12].to_string();
    let hash = sha2_hex(&full_key);

    let model = api_key::ActiveModel {
        id: Set(Uuid::new_v4()),
        key_name: Set(body.key_name),
        key_hash: Set(hash),
        key_prefix: Set(prefix.clone()),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
    };
    let row = model.insert(&state.db).await
        .map_err(|e| err_400(&format!("Insert failed: {}", e)))?;
    state.reload_config().await.ok();

    Ok((StatusCode::CREATED, Json(CreateApiKeyResponse {
        id: row.id.to_string(),
        key_name: row.key_name,
        key_prefix: prefix,
        full_key,
    })))
}

async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::api_key;
    let existing = api_key::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("API key not found"))?;
    existing.delete(&state.db).await.map_err(|_| err_500("Delete failed"))?;
    state.reload_config().await.ok();
    Ok(StatusCode::NO_CONTENT)
}

// ─── Settings ────────────────────────────────────────────────────

async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Json<SettingsResponse> {
    use crate::entities::{server_config, rate_limit_config};

    let sc = server_config::Entity::find_by_id(1).one(&state.db).await
        .unwrap_or(None)
        .unwrap_or(server_config::Model {
            id: 1, host: "0.0.0.0".into(), port: 3000,
            updated_at: chrono::Utc::now(),
        });

    let rl = rate_limit_config::Entity::find_by_id(1).one(&state.db).await
        .unwrap_or(None)
        .unwrap_or(rate_limit_config::Model {
            id: 1, enabled: true, requests_per_minute: 60, burst_size: 20,
            updated_at: chrono::Utc::now(),
        });

    Json(SettingsResponse {
        server: ServerSettingsResponse { host: sc.host, port: sc.port },
        rate_limit: RateLimitSettingsResponse {
            enabled: rl.enabled, requests_per_minute: rl.requests_per_minute,
            burst_size: rl.burst_size,
        },
    })
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::{server_config, rate_limit_config};
    use sea_orm::ActiveValue::Set;

    if let Some(srv) = body.server {
        let existing = server_config::Entity::find_by_id(1).one(&state.db).await
            .map_err(|_| err_500("Database error"))?;
        if let Some(row) = existing {
            let mut model: server_config::ActiveModel = row.into();
            if let Some(v) = srv.host { model.host = Set(v); }
            if let Some(v) = srv.port { model.port = Set(v); }
            model.updated_at = Set(chrono::Utc::now());
            model.update(&state.db).await.map_err(|e| err_400(&e.to_string()))?;
        }
    }

    if let Some(rl) = body.rate_limit {
        let existing = rate_limit_config::Entity::find_by_id(1).one(&state.db).await
            .map_err(|_| err_500("Database error"))?;
        if let Some(row) = existing {
            let mut model: rate_limit_config::ActiveModel = row.into();
            if let Some(v) = rl.enabled { model.enabled = Set(v); }
            if let Some(v) = rl.requests_per_minute { model.requests_per_minute = Set(v); }
            if let Some(v) = rl.burst_size { model.burst_size = Set(v); }
            model.updated_at = Set(chrono::Utc::now());
            model.update(&state.db).await.map_err(|e| err_400(&e.to_string()))?;
        }
    }

    state.reload_config().await.ok();
    Ok(get_settings(State(state)).await)
}

// ─── Error helpers ───────────────────────────────────────────────

fn err_400(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": msg})))
}

fn err_404(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": msg})))
}

fn err_500(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": msg})))
}
