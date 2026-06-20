use serde::{Deserialize, Serialize};

// ─── Existing types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DashboardData {
    pub providers: Vec<ProviderStatus>,
    pub metrics: MetricsData,
    pub models: Vec<ModelInfo>,
    pub live_metrics: LiveMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderStatus {
    pub name: String,
    pub provider_type: String,
    pub model_count: usize,
    pub color: String,
    pub request_count: u64,
    pub error_count: u64,
    pub avg_latency_ms: f64,
    pub healthy: bool,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricsData {
    pub total_providers: usize,
    pub total_models: usize,
    pub built_in_free: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiveMetrics {
    pub total_requests: u64,
    pub total_errors: u64,
    pub avg_latency_ms: f64,
    pub error_rate: f64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

// ─── CRUD Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderDetail {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub category: String,
    pub api_key: String,
    pub base_url: String,
    pub models: Vec<String>,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderTypeInfo {
    pub id: String,
    pub display_name: String,
    pub category: String,
    pub category_label: String,
    pub needs_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteDetail {
    pub id: String,
    pub model: String,
    pub strategy: String,
    pub provider: Option<String>,
    pub providers: Option<Vec<String>>,
    pub combo: serde_json::Value,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKeyDetail {
    pub id: String,
    pub key_name: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyCreateResponse {
    pub id: String,
    pub key_name: String,
    pub key_prefix: String,
    pub full_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingsData {
    pub server: ServerSettingsData,
    pub rate_limit: RateLimitSettingsData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerSettingsData {
    pub host: String,
    pub port: i32,
    pub default_max_tokens: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitSettingsData {
    pub enabled: bool,
    pub requests_per_minute: i64,
    pub burst_size: i32,
}

// ─── Auth token ──────────────────────────────────────────────────

static AUTH_TOKEN: &str = "Bearer sk-test-abc123";

// ─── Generic API helper ──────────────────────────────────────────

async fn api_request<T: serde::de::DeserializeOwned>(
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<T, String> {
    let window = web_sys::window().ok_or("No window")?;
    let mut opts = web_sys::RequestInit::new();
    opts.set_method(method);
    opts.set_mode(web_sys::RequestMode::Cors);
    if let Some(b) = body {
        opts.set_body(&wasm_bindgen::JsValue::from_str(b));
    }

    let request = web_sys::Request::new_with_str_and_init(path, &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;
    request.headers().set("Authorization", AUTH_TOKEN)
        .map_err(|e| format!("Header error: {:?}", e))?;
    if body.is_some() {
        request.headers().set("Content-Type", "application/json")
            .map_err(|e| format!("Header error: {:?}", e))?;
    }

    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = wasm_bindgen::JsCast::dyn_into(resp).map_err(|_| "Type error".to_string())?;

    if !resp.ok() && resp.status() != 201 {
        return Err(format!("HTTP {}", resp.status()));
    }

    let json = wasm_bindgen_futures::JsFuture::from(resp.json().map_err(|e| format!("JSON error: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON parse: {:?}", e))?;

    serde_wasm_bindgen::from_value::<T>(json)
        .map_err(|e| format!("Deserialize: {:?}", e))
}

// ─── Dashboard ───────────────────────────────────────────────────

pub async fn fetch_dashboard() -> Result<DashboardData, String> {
    api_request::<DashboardData>("GET", "/api/dashboard", None).await
}

// ─── Providers CRUD ──────────────────────────────────────────────

pub async fn fetch_provider_types() -> Result<Vec<ProviderTypeInfo>, String> {
    api_request::<Vec<ProviderTypeInfo>>("GET", "/api/dashboard/provider-types", None).await
}

pub async fn fetch_providers() -> Result<Vec<ProviderDetail>, String> {
    api_request::<Vec<ProviderDetail>>("GET", "/api/dashboard/providers", None).await
}

pub async fn create_provider(data: &str) -> Result<ProviderDetail, String> {
    api_request::<ProviderDetail>("POST", "/api/dashboard/providers", Some(data)).await
}

pub async fn update_provider(id: &str, data: &str) -> Result<ProviderDetail, String> {
    api_request::<ProviderDetail>("PUT", &format!("/api/dashboard/providers/{}", id), Some(data)).await
}

pub async fn delete_provider(id: &str) -> Result<(), String> {
    let _: serde_json::Value = api_request("DELETE", &format!("/api/dashboard/providers/{}", id), None).await?;
    Ok(())
}

// ─── Routes CRUD ─────────────────────────────────────────────────

pub async fn fetch_routes() -> Result<Vec<RouteDetail>, String> {
    api_request::<Vec<RouteDetail>>("GET", "/api/dashboard/routes", None).await
}

pub async fn create_route(data: &str) -> Result<RouteDetail, String> {
    api_request::<RouteDetail>("POST", "/api/dashboard/routes", Some(data)).await
}

pub async fn update_route(id: &str, data: &str) -> Result<RouteDetail, String> {
    api_request::<RouteDetail>("PUT", &format!("/api/dashboard/routes/{}", id), Some(data)).await
}

pub async fn delete_route(id: &str) -> Result<(), String> {
    let _: serde_json::Value = api_request("DELETE", &format!("/api/dashboard/routes/{}", id), None).await?;
    Ok(())
}

// ─── API Keys CRUD ───────────────────────────────────────────────

pub async fn fetch_api_keys() -> Result<Vec<ApiKeyDetail>, String> {
    api_request::<Vec<ApiKeyDetail>>("GET", "/api/dashboard/api-keys", None).await
}

pub async fn create_api_key(data: &str) -> Result<ApiKeyCreateResponse, String> {
    api_request::<ApiKeyCreateResponse>("POST", "/api/dashboard/api-keys", Some(data)).await
}

pub async fn delete_api_key(id: &str) -> Result<(), String> {
    let _: serde_json::Value = api_request("DELETE", &format!("/api/dashboard/api-keys/{}", id), None).await?;
    Ok(())
}

// ─── Settings ────────────────────────────────────────────────────

pub async fn fetch_settings() -> Result<SettingsData, String> {
    api_request::<SettingsData>("GET", "/api/dashboard/settings", None).await
}

pub async fn update_settings(data: &str) -> Result<SettingsData, String> {
    api_request::<SettingsData>("PUT", "/api/dashboard/settings", Some(data)).await
}
