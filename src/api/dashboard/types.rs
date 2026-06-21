use serde::{Serialize, Deserialize};
use crate::types::openai::ModelInfo;

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
    pub category: String,
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
    pub category: String,
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
pub struct ServerSettingsResponse {
    pub host: String, pub port: i32, pub default_max_tokens: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitSettingsResponse {
    pub enabled: bool, pub requests_per_minute: i64, pub burst_size: i32,
}

#[derive(Debug, Serialize)]
pub struct ProviderTypeInfo {
    pub id: String,
    pub display_name: String,
    pub category: String,
    pub category_label: String,
    pub needs_api_key: bool,
}

#[derive(Debug, Serialize)]
pub struct ProviderMetricsList {
    pub providers: Vec<crate::tracker::ProviderMetrics>,
    pub global: crate::tracker::GlobalMetrics,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String, pub key_name: String,
    pub key_prefix: String, pub full_key: String,
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
pub struct TestProviderRequest {
    pub model: String,
}

#[derive(Debug, Serialize)]
pub struct TestProviderResponse {
    pub ok: bool,
    pub latency_ms: u64,
    pub model: String,
    pub error: Option<String>,
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

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub server: Option<ServerSettingsUpdate>,
    pub rate_limit: Option<RateLimitSettingsUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettingsUpdate {
    pub host: Option<String>, pub port: Option<i32>, pub default_max_tokens: Option<Option<i32>>,
}

#[derive(Debug, Deserialize)]
pub struct RateLimitSettingsUpdate {
    pub enabled: Option<bool>, pub requests_per_minute: Option<i64>,
    pub burst_size: Option<i32>,
}
