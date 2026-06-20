use async_trait::async_trait;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

/// Ollama — local LLM inference
/// Default endpoint: http://localhost:11434/v1
pub struct OllamaProvider {
    name: String,
    api_key: String,
    base_url: String,
    model_list: Vec<String>,
    client: Client,
}

impl OllamaProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        let base_url = if config.base_url.is_empty() {
            "http://localhost:11434/v1".to_string()
        } else {
            config.base_url.trim_end_matches('/').to_string()
        };
        Self {
            name: config.name.clone(),
            api_key: config.api_key.clone(),
            base_url,
            model_list: config.models.clone(),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "ollama" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut req_builder = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request);

        if !self.api_key.is_empty() {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let resp = req_builder.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;

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
        let url = format!("{}/chat/completions", self.base_url);
        let mut stream_req = request.clone();
        stream_req.stream = Some(true);

        let mut req_builder = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&stream_req);

        if !self.api_key.is_empty() {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = req_builder.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
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
        let url = format!("{}/models", self.base_url);
        let req_builder = self.client.get(&url);

        let resp = req_builder.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;

        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if let Ok(upstream) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = upstream["data"].as_array() {
                        let models: Vec<ModelInfo> = arr.iter().map(|m| ModelInfo {
                            id: m["id"].as_str().unwrap_or("unknown").to_string(),
                            object: "model".to_string(),
                            created: m["created"].as_u64().unwrap_or(0),
                            owned_by: "ollama".to_string(),
                        }).collect();
                        if !models.is_empty() {
                            return Ok(ModelListResponse { object: "list".to_string(), data: models });
                        }
                    }
                }
            }
        }

        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let data: Vec<ModelInfo> = self.model_list.iter().map(|id| ModelInfo {
            id: id.clone(),
            object: "model".to_string(),
            created: ts,
            owned_by: self.name.clone(),
        }).collect();
        Ok(ModelListResponse { object: "list".to_string(), data })
    }
}
