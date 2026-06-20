use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub name: String,
    pub provider_type: String,
    pub model_count: usize,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsData {
    pub total_providers: usize,
    pub total_models: usize,
    pub built_in_free: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub providers: Vec<ProviderStatus>,
    pub metrics: MetricsData,
    pub models: Vec<ModelInfo>,
}

pub async fn fetch_dashboard(api_key: &str) -> Result<DashboardData, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("/api/dashboard")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("API error: {}", resp.status()));
    }
    resp.json().await.map_err(|e| format!("JSON error: {}", e))
}

pub async fn fetch_models(api_key: &str) -> Result<Vec<ModelInfo>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("API error: {}", resp.status()));
    }
    resp.json().await.map_err(|e| format!("JSON error: {}", e))
}
