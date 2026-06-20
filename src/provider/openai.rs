use async_trait::async_trait;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

pub struct OpenAIProvider {
    name: String,
    api_key: String,
    base_url: String,
    model_list: Vec<String>,
    client: Client,
}

impl OpenAIProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            api_key: config.api_key.clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            model_list: config.models.clone(),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "openai" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);
        let resp = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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
        let url = format!("{}/chat/completions", self.base_url);
        let mut req_builder = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream");

        let mut stream_req = request.clone();
        stream_req.stream = Some(true);
        req_builder = req_builder.json(&stream_req);

        let response = req_builder
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        // Read entire SSE response and split into chunks
        let body_bytes = response.bytes().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        let text = String::from_utf8_lossy(&body_bytes);
        let mut chunks = Vec::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    break;
                }
                if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                    chunks.push(Ok(chunk));
                }
            }
        }

        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    async fn list_models(&self) -> Result<ModelListResponse, ProviderError> {
        let url = format!("{}/models", self.base_url);
        let resp = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        let text = resp.text().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        let upstream: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| ProviderError::Http(format!("JSON parse error: {}", e)))?;

        let models: Vec<ModelInfo> = upstream["data"].as_array().map(|arr| {
            arr.iter().map(|m| ModelInfo {
                id: m["id"].as_str().unwrap_or("unknown").to_string(),
                object: "model".to_string(),
                created: m["created"].as_u64().unwrap_or(0),
                owned_by: m["owned_by"].as_str().unwrap_or("airouter").to_string(),
            }).collect()
        }).unwrap_or_default();

        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let fallback: Vec<ModelInfo> = self.model_list.iter().map(|id| ModelInfo {
            id: id.clone(),
            object: "model".to_string(),
            created: ts,
            owned_by: self.name.clone(),
        }).collect();

        let data = if models.is_empty() { fallback } else { models };

        Ok(ModelListResponse { object: "list".to_string(), data })
    }
}
