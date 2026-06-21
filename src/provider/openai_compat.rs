use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

pub struct OpenAICompatProvider {
    name: String,
    provider_type_name: String,
    api_key: String,
    base_url: String,
    model_list: Vec<String>,
    extra_headers: HashMap<String, String>,
    client: Client,
}

impl OpenAICompatProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            provider_type_name: config.provider_type.clone(),
            api_key: config.api_key.clone(),
            base_url: config.base_url.trim_end_matches('/').to_string(),
            model_list: config.models.clone(),
            extra_headers: config.extra_headers.clone(),
            client: Client::new(),
        }
    }

    fn apply_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let mut builder = builder;
        for (k, v) in &self.extra_headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        builder
    }
}

#[async_trait]
impl Provider for OpenAICompatProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { &self.provider_type_name }
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
        req_builder = self.apply_headers(req_builder);

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
        let mut req_builder = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream");

        let mut stream_req = request.clone();
        stream_req.stream = Some(true);
        req_builder = req_builder.json(&stream_req);

        if !self.api_key.is_empty() {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }
        req_builder = self.apply_headers(req_builder);

        let response = req_builder.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;

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
        let mut req_builder = self.client.get(&url);

        if !self.api_key.is_empty() {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", self.api_key));
        }
        req_builder = self.apply_headers(req_builder);

        let resp = req_builder.send().await.map_err(|e| ProviderError::Http(e.to_string()))?;

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
