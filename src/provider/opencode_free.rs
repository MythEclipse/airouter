use async_trait::async_trait;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

/// OpenCode Free — no API key required
/// Uses `Authorization: Bearer public` + `x-opencode-client: desktop`
/// Endpoint: https://opencode.ai/zen/v1
/// Models fetched dynamically from https://opencode.ai/zen/v1/models
pub struct OpenCodeFreeProvider {
    name: String,
    model_list: Vec<String>,
    client: Client,
}

impl OpenCodeFreeProvider {
    pub fn new(_config: &ProviderConfig) -> Self {
        Self {
            name: _config.name.clone(),
            model_list: _config.models.clone(),
            client: Client::new(),
        }
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Authorization", "Bearer public".parse().unwrap());
        headers.insert("x-opencode-client", "desktop".parse().unwrap());
        headers
    }
}

#[async_trait]
impl Provider for OpenCodeFreeProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "opencode_free" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        let url = "https://opencode.ai/zen/v1/chat/completions";
        let resp = self.client
            .post(url)
            .headers(self.build_headers())
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        let text = resp.text().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        serde_json::from_str::<ChatCompletionResponse>(&text)
            .map_err(|e| ProviderError::Http(format!("JSON parse error: {}", e)))
    }

    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<ProviderStream, ProviderError> {
        let url = "https://opencode.ai/zen/v1/chat/completions";
        let mut stream_req = request.clone();
        stream_req.stream = Some(true);

        let response = self.client
            .post(url)
            .headers(self.build_headers())
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&stream_req)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        let body_bytes = response.bytes().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        let text = String::from_utf8_lossy(&body_bytes);
        let mut chunks = Vec::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" { break; }
                if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                    chunks.push(Ok(chunk));
                }
            }
        }

        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    async fn list_models(&self) -> Result<ModelListResponse, ProviderError> {
        // Fetch live models from opencode.ai
        let url = "https://opencode.ai/zen/v1/models";
        let resp = self.client
            .get(url)
            .headers(self.build_headers())
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if let Ok(upstream) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = upstream["data"].as_array() {
                        let models: Vec<ModelInfo> = arr.iter().map(|m| ModelInfo {
                            id: m["id"].as_str().unwrap_or("unknown").to_string(),
                            object: "model".to_string(),
                            created: m["created"].as_u64().unwrap_or(0),
                            owned_by: "opencode".to_string(),
                        }).collect();
                        if !models.is_empty() {
                            return Ok(ModelListResponse { object: "list".to_string(), data: models });
                        }
                    }
                }
            }
        }

        // Fallback to config list
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let data: Vec<ModelInfo> = self.model_list.iter().map(|id| ModelInfo {
            id: id.clone(),
            object: "model".to_string(),
            created: ts,
            owned_by: "opencode".to_string(),
        }).collect();
        Ok(ModelListResponse { object: "list".to_string(), data })
    }
}
