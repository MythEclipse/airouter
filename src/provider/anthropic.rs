use async_trait::async_trait;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

fn convert_chat_error(e: String) -> ProviderError {
    ProviderError::Http(e)
}

fn convert_response(
    anthro_resp: crate::types::anthropic::MessagesResponse,
    model: &str,
) -> Result<ChatCompletionResponse, ProviderError> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let mut content = String::new();

    for block in &anthro_resp.content {
        if block.block_type == "text" {
            if let Some(text) = &block.text {
                content.push_str(text);
            }
        }
    }

    let finish_reason = anthro_resp.stop_reason.as_deref().map(|s| match s {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        other => other,
    }).map(|s| s.to_string());

    Ok(ChatCompletionResponse {
        id: anthro_resp.id.clone(),
        object: "chat.completion".into(),
        created: ts,
        model: model.to_string(),
        choices: vec![Choice {
            index: 0,
            message: ResponseMessage {
                role: "assistant".into(),
                content: Some(content),
                tool_calls: None,
            },
            finish_reason,
            logprobs: None,
        }],
        usage: Some(Usage {
            prompt_tokens: anthro_resp.usage.input_tokens,
            completion_tokens: anthro_resp.usage.output_tokens,
            total_tokens: anthro_resp.usage.input_tokens + anthro_resp.usage.output_tokens,
        }),
    })
}

pub struct AnthropicProvider {
    name: String,
    api_key: String,
    base_url: String,
    model_list: Vec<String>,
    client: Client,
}

impl AnthropicProvider {
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
impl Provider for AnthropicProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "anthropic" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        let anthro_req = crate::transform::openai_to_anthropic::convert_chat_request(&request)
            .map_err(convert_chat_error)?;
        let url = format!("{}/messages", self.base_url);

        let resp = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthro_req)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        let text = resp.text().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        let anthro_resp: crate::types::anthropic::MessagesResponse = serde_json::from_str(&text)
            .map_err(|e| ProviderError::Http(format!("JSON parse error: {}", e)))?;

        convert_response(anthro_resp, &request.model)
    }

    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<ProviderStream, ProviderError> {
        let anthro_req = crate::transform::openai_to_anthropic::convert_chat_request(&request)
            .map_err(convert_chat_error)?;
        let url = format!("{}/messages", self.base_url);

        let mut stream_req = anthro_req.clone();
        stream_req.stream = Some(true);

        let response = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
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
        let model_name = request.model.clone();
        let mut chunks = Vec::new();

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                // Parse anthropic SSE events and convert to OpenAI chunks
                if let Ok(msg_start) = serde_json::from_str::<crate::types::anthropic::MessageStartEvent>(data) {
                    chunks.push(Ok(ChatCompletionChunk {
                        id: msg_start.message.id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        model: model_name.clone(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: Some(Delta { role: Some("assistant".into()), content: None, tool_calls: None }),
                            finish_reason: None,
                        }],
                        usage: None,
                    }));
                } else if let Ok(delta) = serde_json::from_str::<crate::types::anthropic::ContentBlockDeltaEvent>(data) {
                    if delta.delta.delta_type == "text_delta" {
                        chunks.push(Ok(ChatCompletionChunk {
                            id: String::new(),
                            object: "chat.completion.chunk".to_string(),
                            created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                            model: model_name.clone(),
                            choices: vec![ChunkChoice {
                                index: delta.index,
                                delta: Some(Delta { role: None, content: delta.delta.text.clone(), tool_calls: None }),
                                finish_reason: None,
                            }],
                            usage: None,
                        }));
                    }
                } else if let Ok(msg_delta) = serde_json::from_str::<crate::types::anthropic::MessageDeltaEvent>(data) {
                    let finish = msg_delta.delta.stop_reason.clone().unwrap_or_default();
                    let finish_reason = if finish == "end_turn" || finish == "stop_sequence" {
                        Some("stop".to_string())
                    } else if finish == "max_tokens" {
                        Some("length".to_string())
                    } else if finish == "tool_use" {
                        Some("tool_calls".to_string())
                    } else { None };

                    let usage = msg_delta.usage;
                    chunks.push(Ok(ChatCompletionChunk {
                        id: String::new(),
                        object: "chat.completion.chunk".to_string(),
                        created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        model: model_name.clone(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: Some(Delta { role: None, content: None, tool_calls: None }),
                            finish_reason,
                        }],
                        usage: Some(Usage {
                            prompt_tokens: usage.input_tokens,
                            completion_tokens: usage.output_tokens,
                            total_tokens: usage.input_tokens + usage.output_tokens,
                        }),
                    }));
                }
            }
        }

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
