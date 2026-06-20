use serde::{Deserialize, Serialize};

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

/// Fetch dashboard data from the backend API
pub async fn fetch_dashboard() -> Result<DashboardData, String> {
    let window = web_sys::window().ok_or("No window".to_string())?;
    let mut opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let request = web_sys::Request::new_with_str_and_init("/api/dashboard", &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;
    request.headers().set("Authorization", "Bearer sk-test-abc123")
        .map_err(|e| format!("Header error: {:?}", e))?;

    let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = wasm_bindgen::JsCast::dyn_into(resp).map_err(|_| "Type error".to_string())?;

    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let json = wasm_bindgen_futures::JsFuture::from(resp.json().map_err(|e| format!("JSON error: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON parse: {:?}", e))?;

    serde_wasm_bindgen::from_value::<DashboardData>(json)
        .map_err(|e| format!("Deserialize: {:?}", e))
}
