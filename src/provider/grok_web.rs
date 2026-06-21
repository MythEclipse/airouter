use async_trait::async_trait;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

/// Grok Web — cookie-based access to grok.com REST API.
/// Auth: Cookie header with `sso=<cookie_value>`.
/// Endpoint: https://grok.com/rest/app-chat/conversations/new
/// Response: ndjson stream with `result.response.token` or `result.response.modelResponse` fields.
pub struct GrokWebProvider {
    name: String,
    cookie_value: String,
    model_list: Vec<String>,
    client: Client,
}

/// Maps friendly model names to grok.com internal model + mode.
struct GrokModelInfo {
    grok_model: &'static str,
    model_mode: &'static str,
}

const DEFAULT_MODEL: GrokModelInfo = GrokModelInfo {
    grok_model: "grok-4",
    model_mode: "MODEL_MODE_GROK_4",
};

fn grok_model_lookup(model: &str) -> GrokModelInfo {
    match model {
        "grok-3" => GrokModelInfo { grok_model: "grok-3", model_mode: "MODEL_MODE_GROK_3" },
        "grok-3-mini" => GrokModelInfo { grok_model: "grok-3", model_mode: "MODEL_MODE_GROK_3_MINI_THINKING" },
        "grok-3-thinking" => GrokModelInfo { grok_model: "grok-3", model_mode: "MODEL_MODE_GROK_3_THINKING" },
        "grok-4" => GrokModelInfo { grok_model: "grok-4", model_mode: "MODEL_MODE_GROK_4" },
        "grok-4-mini" => GrokModelInfo { grok_model: "grok-4-mini", model_mode: "MODEL_MODE_GROK_4_MINI_THINKING" },
        "grok-4-thinking" => GrokModelInfo { grok_model: "grok-4", model_mode: "MODEL_MODE_GROK_4_THINKING" },
        "grok-4-heavy" => GrokModelInfo { grok_model: "grok-4", model_mode: "MODEL_MODE_HEAVY" },
        "grok-4.1-mini" => GrokModelInfo { grok_model: "grok-4-1-thinking-1129", model_mode: "MODEL_MODE_GROK_4_1_MINI_THINKING" },
        "grok-4.1-fast" => GrokModelInfo { grok_model: "grok-4-1-thinking-1129", model_mode: "MODEL_MODE_FAST" },
        "grok-4.1-expert" => GrokModelInfo { grok_model: "grok-4-1-thinking-1129", model_mode: "MODEL_MODE_EXPERT" },
        "grok-4.1-thinking" => GrokModelInfo { grok_model: "grok-4-1-thinking-1129", model_mode: "MODEL_MODE_GROK_4_1_THINKING" },
        "grok-4.2" | "grok-4.20" => GrokModelInfo { grok_model: "grok-420", model_mode: "MODEL_MODE_GROK_420" },
        _ => DEFAULT_MODEL,
    }
}

/// Check if a model mode indicates a thinking model.
fn is_thinking_model(model_mode: &str) -> bool {
    model_mode.contains("THINKING") || model_mode.contains("HEAVY")
        || model_mode.contains("EXPERT")
}

/// Convert OpenAI messages into a single string prompt for grok.com.
fn parse_openai_messages(messages: &[Message]) -> String {
    let mut extracted: Vec<(&str, String)> = Vec::new();
    for msg in messages {
        let role = match msg.role.as_str() {
            "developer" => "system",
            r => r,
        };
        let content = match &msg.content {
            Some(Content::Text(s)) => s.clone(),
            Some(Content::Parts(parts)) => parts.iter()
                .filter_map(|p| p.text.as_deref())
                .collect::<Vec<_>>()
                .join(" "),
            None => String::new(),
        };
        if !content.trim().is_empty() {
            extracted.push((role, content));
        }
    }
    if extracted.is_empty() {
        return String::new();
    }
    // Find last user message index
    let last_user_idx = extracted.iter().rposition(|(r, _)| *r == "user").unwrap_or(0);
    let mut parts = Vec::new();
    for (i, (role, text)) in extracted.iter().enumerate() {
        if i == last_user_idx {
            parts.push(text.clone());
        } else {
            parts.push(format!("{}: {}", role, text));
        }
    }
    parts.join("\n\n")
}

/// Build the grok.com ndjson request payload.
fn build_grok_payload(model: &str, message: String) -> serde_json::Value {
    let info = grok_model_lookup(model);
    let is_thinking = is_thinking_model(info.model_mode);
    serde_json::json!({
        "temporary": true,
        "modelName": info.grok_model,
        "modelMode": info.model_mode,
        "message": message,
        "fileAttachments": [],
        "imageAttachments": [],
        "disableSearch": false,
        "enableImageGeneration": false,
        "returnImageBytes": false,
        "returnRawGrokInXaiRequest": false,
        "enableImageStreaming": false,
        "imageGenerationCount": 0,
        "forceConcise": false,
        "toolOverrides": {},
        "enableSideBySide": true,
        "sendFinalMetadata": true,
        "isReasoning": is_thinking,
        "disableTextFollowUps": false,
        "disableMemory": true,
        "forceSideBySide": false,
        "isAsyncChat": false,
        "disableSelfHarmShortCircuit": false,
        "deviceEnvInfo": {
            "darkModeEnabled": false,
            "devicePixelRatio": 2,
            "screenWidth": 1920,
            "screenHeight": 1080,
            "viewportWidth": 1920,
            "viewportHeight": 1080
        }
    })
}

/// Generate a random hex string of `n` bytes (2n hex chars).
fn random_hex(n: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..n).map(|_| format!("{:02x}", rng.gen::<u8>())).collect()
}

/// Build browser-like headers for grok.com.
fn build_headers(cookie_value: &str) -> Vec<(String, String)> {
    let mut token = cookie_value.to_string();
    if token.starts_with("sso=") {
        token = token[4..].to_string();
    }
    let trace_id = random_hex(16);
    let span_id = random_hex(8);
    let request_id = uuid::Uuid::new_v4().to_string();

    vec![
        ("Accept".into(), "*/*".into()),
        ("Accept-Language".into(), "en-US,en;q=0.9".into()),
        ("Cache-Control".into(), "no-cache".into()),
        ("Content-Type".into(), "application/json".into()),
        ("Origin".into(), "https://grok.com".into()),
        ("Pragma".into(), "no-cache".into()),
        ("Referer".into(), "https://grok.com/".into()),
        ("User-Agent".into(),
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36".into()),
        ("Cookie".into(), format!("sso={}", token)),
        ("x-xai-request-id".into(), request_id),
        ("traceparent".into(), format!("00-{}-{}-00", trace_id, span_id)),
    ]
}

impl GrokWebProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            cookie_value: config.api_key.clone(),
            model_list: config.models.clone(),
            client: Client::new(),
        }
    }

    fn chat_url(&self) -> String {
        "https://grok.com/rest/app-chat/conversations/new".to_string()
    }

    /// Send request to grok.com, return (status, response body text).
    async fn send_request(&self, payload: serde_json::Value) -> Result<(u16, String), ProviderError> {
        let mut builder = self.client
            .post(self.chat_url())
            .header("Content-Type", "application/json")
            .body(payload.to_string());

        for (k, v) in build_headers(&self.cookie_value) {
            builder = builder.header(k.as_str(), v.as_str());
        }

        let resp = builder.send().await
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        Ok((status, text))
    }

    /// Send streaming request and return the raw response for byte-level SSE/ndjson parsing.
    async fn send_stream_request(&self, payload: serde_json::Value) -> Result<reqwest::Response, ProviderError> {
        let mut builder = self.client
            .post(self.chat_url())
            .header("Content-Type", "application/json")
            .body(payload.to_string());

        for (k, v) in build_headers(&self.cookie_value) {
            builder = builder.header(k.as_str(), v.as_str());
        }

        let resp = builder.send().await
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        Ok(resp)
    }
}

#[async_trait]
impl Provider for GrokWebProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "grok_web" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        if self.cookie_value.is_empty() {
            return Err(ProviderError::Unavailable("Grok Web: no cookie configured (set api_key to sso cookie value)".into()));
        }

        let message = parse_openai_messages(&request.messages);
        if message.trim().is_empty() {
            return Err(ProviderError::Api { status: 400, body: "Empty message after processing".into() });
        }

        let payload = build_grok_payload(&request.model, message);
        let (status, text) = self.send_request(payload).await?;

        if status != 200 {
            let msg = if status == 401 || status == 429 {
                "Grok auth failed — SSO cookie may be expired. Re-paste your sso cookie."
            } else {
                "Grok upstream error"
            };
            return Err(ProviderError::Api { status, body: msg.into() });
        }

        // Parse ndjson response — collect all tokens and full messages
        let mut full_content = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(resp) = val.pointer("/result/response") {
                    if let Some(msg) = resp.pointer("/modelResponse/message").and_then(|v| v.as_str()) {
                        full_content = msg.to_string();
                    }
                    if let Some(token) = resp.get("token").and_then(|v| v.as_str()) {
                        full_content.push_str(token);
                    }
                }
            }
        }

        let id = format!("chatcmpl-grok-{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..12].to_string());
        let created = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let prompt_tokens = (request.messages.len() * 20) as u32;
        let completion_tokens = (full_content.len() / 4).max(1) as u32;

        Ok(ChatCompletionResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model: request.model,
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content: Some(full_content),
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            }),
        })
    }

    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<ProviderStream, ProviderError> {
        if self.cookie_value.is_empty() {
            return Err(ProviderError::Unavailable("Grok Web: no cookie configured (set api_key to sso cookie value)".into()));
        }

        let message = parse_openai_messages(&request.messages);
        if message.trim().is_empty() {
            return Err(ProviderError::Api { status: 400, body: "Empty message after processing".into() });
        }

        let payload = build_grok_payload(&request.model, message);
        let resp = self.send_stream_request(payload).await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let cid = format!("chatcmpl-grok-{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..12].to_string());
        let created = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let model = request.model.clone();

        let body_bytes = resp.bytes().await
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        let text = String::from_utf8_lossy(&body_bytes);

        let mut chunks: Vec<Result<ChatCompletionChunk, ProviderError>> = Vec::new();

        // First chunk: role
        chunks.push(Ok(ChatCompletionChunk {
            id: cid.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Some(Delta {
                    role: Some("assistant".to_string()),
                    content: None,
                    tool_calls: None,
                }),
                finish_reason: None,
            }],
            usage: None,
        }));

        // Parse ndjson lines — grok returns one JSON object per line
        let mut content_buf = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if let Some(resp) = val.pointer("/result/response") {
                    // Token-by-token streaming
                    if let Some(token) = resp.get("token").and_then(|v| v.as_str()) {
                        if !token.is_empty() {
                            chunks.push(Ok(ChatCompletionChunk {
                                id: cid.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model.clone(),
                                choices: vec![ChunkChoice {
                                    index: 0,
                                    delta: Some(Delta {
                                        role: None,
                                        content: Some(token.to_string()),
                                        tool_calls: None,
                                    }),
                                    finish_reason: None,
                                }],
                                usage: None,
                            }));
                            content_buf.push_str(token);
                        }
                    }
                    // Full message response (used for some models)
                    if let Some(msg) = resp.pointer("/modelResponse/message").and_then(|v| v.as_str()) {
                        if !msg.is_empty() {
                            // This is the complete message — emit it as a single chunk
                            chunks.push(Ok(ChatCompletionChunk {
                                id: cid.clone(),
                                object: "chat.completion.chunk".to_string(),
                                created,
                                model: model.clone(),
                                choices: vec![ChunkChoice {
                                    index: 0,
                                    delta: Some(Delta {
                                        role: None,
                                        content: Some(msg.to_string()),
                                        tool_calls: None,
                                    }),
                                    finish_reason: None,
                                }],
                                usage: None,
                            }));
                            content_buf = msg.to_string();
                        }
                    }
                }
            }
        }

        // Final chunk: stop
        chunks.push(Ok(ChatCompletionChunk {
            id: cid.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Some(Delta {
                    role: None,
                    content: None,
                    tool_calls: None,
                }),
                finish_reason: Some("stop".to_string()),
            }],
            usage: None,
        }));

        Ok(Box::pin(futures::stream::iter(chunks)))
    }

    async fn list_models(&self) -> Result<ModelListResponse, ProviderError> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let data: Vec<ModelInfo> = self.model_list.iter().map(|id| ModelInfo {
            id: id.clone(),
            object: "model".to_string(),
            created: ts,
            owned_by: "grok-web".to_string(),
        }).collect();
        Ok(ModelListResponse { object: "list".to_string(), data })
    }
}
