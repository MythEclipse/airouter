use async_trait::async_trait;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

/// Google Gemini via OpenAI-compatible proxy
/// Uses the Gemini API directly with format translation
/// Endpoint: https://generativelanguage.googleapis.com/v1beta
pub struct GeminiProvider {
    name: String,
    api_key: String,
    model_list: Vec<String>,
    client: Client,
}

impl GeminiProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            api_key: config.api_key.clone(),
            model_list: config.models.clone(),
            client: Client::new(),
        }
    }

    /// Convert OpenAI request to Gemini format and call via OpenAI-compatible gateway
    fn gemini_url(&self, model: &str) -> String {
        let model = model.replace('.', "-");
        if !self.api_key.is_empty() {
            format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, self.api_key)
        } else {
            format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent", model)
        }
    }

    fn gemini_stream_url(&self, model: &str) -> String {
        let model = model.replace('.', "-");
        if !self.api_key.is_empty() {
            format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", model, self.api_key)
        } else {
            format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse", model)
        }
    }

    /// Convert OpenAI messages to Gemini contents format
    fn to_gemini_request(&self, request: &ChatCompletionRequest) -> serde_json::Value {
        let mut contents = Vec::new();
        let mut system_text = String::new();

        for msg in &request.messages {
            match msg.role.as_str() {
                "system" => {
                    if let Some(content) = &msg.content {
                        let text = match content {
                            Content::Text(t) => t.clone(),
                            Content::Parts(parts) => {
                                parts.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join("\n")
                            }
                        };
                        system_text.push_str(&text);
                        system_text.push('\n');
                    }
                }
                role => {
                    let text = match &msg.content {
                        Some(Content::Text(t)) => t.clone(),
                        Some(Content::Parts(parts)) => {
                            parts.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join("\n")
                        }
                        None => String::new(),
                    };
                    contents.push(serde_json::json!({
                        "role": role,
                        "parts": [{"text": text}]
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "contents": contents,
            "generationConfig": {
                "temperature": request.temperature.unwrap_or(0.7),
                "maxOutputTokens": request.max_tokens.unwrap_or(4096),
                "topP": request.top_p.unwrap_or(0.95),
            }
        });

        if !system_text.is_empty() {
            body["system_instruction"] = serde_json::json!({
                "parts": [{"text": system_text.trim()}]
            });
        }

        body
    }

    /// Convert Gemini response back to OpenAI format
    fn from_gemini_response(&self, resp: serde_json::Value, model: &str) -> Result<ChatCompletionResponse, ProviderError> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let text = resp["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let finish = resp["candidates"][0]["finishReason"].as_str().unwrap_or("STOP").to_string();
        let finish_reason = match finish.as_str() {
            "STOP" => "stop",
            "MAX_TOKENS" => "length",
            "SAFETY" => "content_filter",
            "RECITATION" => "content_filter",
            _ => "stop",
        };

        Ok(ChatCompletionResponse {
            id: format!("gemini-{}", ts),
            object: "chat.completion".into(),
            created: ts,
            model: model.to_string(),
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: Some(text),
                    tool_calls: None,
                },
                finish_reason: Some(finish_reason.to_string()),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: resp["usageMetadata"]["promptTokenCount"].as_u64().unwrap_or(0) as u32,
                completion_tokens: resp["usageMetadata"]["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
                total_tokens: resp["usageMetadata"]["totalTokenCount"].as_u64().unwrap_or(0) as u32,
            }),
        })
    }

    /// Parse Gemini SSE stream into OpenAI chunks
    fn parse_gemini_stream(&self, text: &str, model: &str) -> Vec<Result<ChatCompletionChunk, ProviderError>> {
        let mut chunks = Vec::new();
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let mut index = 0u32;

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" { break; }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(candidates) = val["candidates"].as_array() {
                        for candidate in candidates {
                            if let Some(text_content) = candidate["content"]["parts"][0]["text"].as_str() {
                                chunks.push(Ok(ChatCompletionChunk {
                                    id: format!("gemini-{}", ts),
                                    object: "chat.completion.chunk".to_string(),
                                    created: ts,
                                    model: model.to_string(),
                                    choices: vec![ChunkChoice {
                                        index,
                                        delta: Some(Delta { role: None, content: Some(text_content.to_string()), tool_calls: None }),
                                        finish_reason: None,
                                    }],
                                    usage: None,
                                }));
                                index += 1;
                            }

                            let finish = candidate["finishReason"].as_str().unwrap_or("");
                            if !finish.is_empty() {
                                let fr = match finish {
                                    "STOP" => "stop",
                                    "MAX_TOKENS" => "length",
                                    _ => "stop",
                                };
                                chunks.push(Ok(ChatCompletionChunk {
                                    id: format!("gemini-{}", ts),
                                    object: "chat.completion.chunk".to_string(),
                                    created: ts,
                                    model: model.to_string(),
                                    choices: vec![ChunkChoice {
                                        index,
                                        delta: None,
                                        finish_reason: Some(fr.to_string()),
                                    }],
                                    usage: None,
                                }));
                            }
                        }
                    }
                }
            }
        }

        chunks
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "gemini" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        let gemini_req = self.to_gemini_request(&request);
        let url = self.gemini_url(&request.model);

        let resp = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&gemini_req)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        let text = resp.text().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        let val: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| ProviderError::Http(format!("JSON parse: {}", e)))?;

        self.from_gemini_response(val, &request.model)
    }

    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<ProviderStream, ProviderError> {
        let gemini_req = self.to_gemini_request(&request);
        let url = self.gemini_stream_url(&request.model);

        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&gemini_req)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let stream_status = response.status();
        if !stream_status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: stream_status.as_u16(), body });
        }

        let body_bytes = response.bytes().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        let text = String::from_utf8_lossy(&body_bytes);
        let chunks = self.parse_gemini_stream(&text, &request.model);

        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    async fn list_models(&self) -> Result<ModelListResponse, ProviderError> {
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
