use async_trait::async_trait;
use reqwest::Client;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

/// Perplexity Web — cookie-based access to perplexity.ai SSE endpoint.
/// Auth: Cookie header with `__Secure-next-auth.session-token=<cookie_value>`.
/// Endpoint: https://www.perplexity.ai/rest/sse/perplexity_ask
/// Response: SSE stream with JSON blocks containing markdown chunks.
pub struct PerplexityWebProvider {
    name: String,
    cookie_value: String,
    model_list: Vec<String>,
    client: Client,
}

const PPLX_SSE_ENDPOINT: &str = "https://www.perplexity.ai/rest/sse/perplexity_ask";
const PPLX_API_VERSION: &str = "2.18";
const PPLX_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// Maps friendly model names to perplexity backend preferences.
struct PplxModelInfo {
    mode: &'static str,
    model_pref: &'static str,
}

const DEFAULT_MODEL: PplxModelInfo = PplxModelInfo {
    mode: "copilot",
    model_pref: "pplx_pro",
};

fn pplx_model_lookup(model: &str) -> PplxModelInfo {
    match model {
        "pplx-auto" => PplxModelInfo { mode: "concise", model_pref: "pplx_pro" },
        "pplx-sonar" => PplxModelInfo { mode: "copilot", model_pref: "experimental" },
        "pplx-gpt" => PplxModelInfo { mode: "copilot", model_pref: "gpt54" },
        "pplx-gemini" => PplxModelInfo { mode: "copilot", model_pref: "gemini31pro_high" },
        "pplx-sonnet" => PplxModelInfo { mode: "copilot", model_pref: "claude46sonnet" },
        "pplx-opus" => PplxModelInfo { mode: "copilot", model_pref: "claude46opus" },
        "pplx-nemotron" => PplxModelInfo { mode: "copilot", model_pref: "nv_nemotron_3_super" },
        _ => DEFAULT_MODEL,
    }
}

/// Convert OpenAI messages to perplexity's format.
fn parse_openai_messages(messages: &[Message]) -> (String, Vec<serde_json::Value>) {
    let mut system_parts: Vec<String> = Vec::new();
    let mut history: Vec<serde_json::Value> = Vec::new();

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
        if content.trim().is_empty() { continue; }
        if role == "system" {
            system_parts.push(content);
        } else if role == "user" || role == "assistant" {
            history.push(serde_json::json!({ "role": role, "content": content }));
        }
    }

    // Last user message becomes the current query
    if let Some(last) = history.last() {
        if last["role"] == "user" {
            // current_msg handled by caller
            history.pop();
        }
    }

    (system_parts.join("\n"), history)
}

/// Build the perplexity SSE request body.
fn build_pplx_body(query: &str, model_info: &PplxModelInfo, follow_up_uuid: Option<&str>) -> serde_json::Value {
    let tz = "UTC"; // simplified — real impl could use chrono
    serde_json::json!({
        "query_str": query,
        "params": {
            "query_str": query,
            "search_focus": "internet",
            "mode": model_info.mode,
            "model_preference": model_info.model_pref,
            "sources": ["web"],
            "attachments": [],
            "frontend_uuid": uuid::Uuid::new_v4().to_string(),
            "frontend_context_uuid": uuid::Uuid::new_v4().to_string(),
            "version": PPLX_API_VERSION,
            "language": "en-US",
            "timezone": tz,
            "search_recency_filter": null,
            "is_incognito": true,
            "use_schematized_api": true,
            "last_backend_uuid": follow_up_uuid,
        }
    })
}

/// Build the query string from parsed messages (combining system + history + current).
fn build_query(system_msg: &str, history: &[serde_json::Value], current_msg: &str) -> String {
    let obj = serde_json::json!({
        "instructions": if system_msg.is_empty() { vec![] } else { vec![system_msg] },
        "history": history,
        "query": current_msg,
    });
    let json = obj.to_string();
    // Truncate if too long
    if json.len() > 96000 {
        json[json.len() - 96000..].to_string()
    } else {
        json
    }
}

/// Clean response text — strip XML-like tags, citations, excessive whitespace.
fn clean_response(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Strip citation markers like [1], [2], [12]
        if bytes[i] == b'[' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() { j += 1; }
            if j > i + 1 && j < bytes.len() && bytes[j] == b']' {
                i = j + 1;
                continue;
            }
        }
        // Strip XML-like tags
        if bytes[i] == b'<' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != b'>' { j += 1; }
            if j < bytes.len() && bytes[j] == b'>' {
                i = j + 1;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    // Collapse 3+ newlines into 2
    let mut out = String::with_capacity(result.len());
    let mut nl_count = 0u32;
    for ch in result.chars() {
        if ch == '\n' {
            nl_count += 1;
            if nl_count <= 2 {
                out.push(ch);
            }
        } else {
            nl_count = 0;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

impl PerplexityWebProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            cookie_value: config.api_key.clone(),
            model_list: config.models.clone(),
            client: Client::new(),
        }
    }

    fn build_headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![
            ("Content-Type".into(), "application/json".into()),
            ("Accept".into(), "text/event-stream".into()),
            ("Origin".into(), "https://www.perplexity.ai".into()),
            ("Referer".into(), "https://www.perplexity.ai/".into()),
            ("User-Agent".into(), PPLX_USER_AGENT.into()),
            ("X-App-ApiClient".into(), "default".into()),
            ("X-App-ApiVersion".into(), PPLX_API_VERSION.into()),
        ];
        if !self.cookie_value.is_empty() {
            let mut token = self.cookie_value.clone();
            if token.starts_with("__Secure-next-auth.session-token=") {
                token = token["__Secure-next-auth.session-token=".len()..].to_string();
            }
            headers.push(("Cookie".into(), format!("__Secure-next-auth.session-token={}", token)));
        }
        headers
    }

    /// Send request and return (status, response text).
    async fn send_request(&self, payload: serde_json::Value) -> Result<(u16, String), ProviderError> {
        let mut builder = self.client
            .post(PPLX_SSE_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .header("Origin", "https://www.perplexity.ai")
            .header("Referer", "https://www.perplexity.ai/")
            .header("User-Agent", PPLX_USER_AGENT)
            .header("X-App-ApiClient", "default")
            .header("X-App-ApiVersion", PPLX_API_VERSION)
            .body(payload.to_string());

        if !self.cookie_value.is_empty() {
            let mut token = self.cookie_value.clone();
            if token.starts_with("__Secure-next-auth.session-token=") {
                token = token["__Secure-next-auth.session-token=".len()..].to_string();
            }
            builder = builder.header("Cookie", format!("__Secure-next-auth.session-token={}", token));
        }

        let resp = builder.send().await
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        Ok((status, text))
    }

    /// Send streaming request and return the raw response.
    async fn send_stream_request(&self, payload: serde_json::Value) -> Result<reqwest::Response, ProviderError> {
        let mut builder = self.client
            .post(PPLX_SSE_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .header("Origin", "https://www.perplexity.ai")
            .header("Referer", "https://www.perplexity.ai/")
            .header("User-Agent", PPLX_USER_AGENT)
            .header("X-App-ApiClient", "default")
            .header("X-App-ApiVersion", PPLX_API_VERSION)
            .body(payload.to_string());

        if !self.cookie_value.is_empty() {
            let mut token = self.cookie_value.clone();
            if token.starts_with("__Secure-next-auth.session-token=") {
                token = token["__Secure-next-auth.session-token=".len()..].to_string();
            }
            builder = builder.header("Cookie", format!("__Secure-next-auth.session-token={}", token));
        }

        let resp = builder.send().await
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        Ok(resp)
    }
}

#[async_trait]
impl Provider for PerplexityWebProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "perplexity_web" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        if self.cookie_value.is_empty() {
            return Err(ProviderError::Unavailable(
                "Perplexity Web: no cookie configured (set api_key to session token)".into()
            ));
        }

        let (system_msg, history) = parse_openai_messages(&request.messages);
        let current_msg = {
            let mut msg = String::new();
            if let Some(last) = request.messages.last() {
                if last.role == "user" {
                    msg = match &last.content {
                        Some(Content::Text(s)) => s.clone(),
                        Some(Content::Parts(parts)) => parts.iter()
                            .filter_map(|p| p.text.as_deref())
                            .collect::<Vec<_>>()
                            .join(" "),
                        None => String::new(),
                    };
                }
            }
            msg
        };

        if current_msg.trim().is_empty() && history.is_empty() {
            return Err(ProviderError::Api { status: 400, body: "Empty query after processing".into() });
        }

        let model_info = pplx_model_lookup(&request.model);
        let query = build_query(&system_msg, &history, &current_msg);
        let payload = build_pplx_body(&query, &model_info, None);

        let (status, text) = self.send_request(payload).await?;

        if status == 401 || status == 403 {
            return Err(ProviderError::Api {
                status,
                body: "Perplexity auth failed — session cookie may be expired.".into(),
            });
        }
        if status != 200 {
            return Err(ProviderError::Api { status, body: text });
        }

        // Parse SSE response — extract content from markdown blocks
        let mut full_answer = String::new();
        let mut data_lines: Vec<String> = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("data:") {
                data_lines.push(trimmed[5..].trim_start().to_string());
            } else if trimmed.is_empty() && !data_lines.is_empty() {
                // Blank line = end of event — flush accumulated data lines
                let payload = data_lines.join("\n");
                data_lines.clear();
                if payload.trim() == "[DONE]" || payload.trim() == "end_of_stream" {
                    break;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload) {
                    if let Some(blocks) = val.get("blocks").and_then(|v| v.as_array()) {
                        for block in blocks {
                            if let Some(mb) = block.get("markdown_block") {
                                if let Some(chunks) = mb.get("chunks").and_then(|v| v.as_array()) {
                                    if mb.get("progress").and_then(|v| v.as_str()) == Some("DONE") {
                                        full_answer = chunks.iter()
                                            .filter_map(|c| c.as_str())
                                            .collect::<Vec<_>>()
                                            .join("");
                                    } else {
                                        let chunk_text: String = chunks.iter()
                                            .filter_map(|c| c.as_str())
                                            .collect::<Vec<_>>()
                                            .join("");
                                        if !chunk_text.is_empty() {
                                            full_answer.push_str(&chunk_text);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(text_val) = val.get("text").and_then(|v| v.as_str()) {
                        if text_val.len() > full_answer.len() {
                            full_answer = text_val.to_string();
                        }
                    }
                    if val.get("final").and_then(|v| v.as_bool()) == Some(true)
                        || val.get("status").and_then(|v| v.as_str()) == Some("COMPLETED")
                    {
                        break;
                    }
                }
            }
        }

        let cleaned = clean_response(&full_answer);
        let id = format!("chatcmpl-pplx-{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..12].to_string());
        let created = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let prompt_tokens = (current_msg.len() / 4).max(1) as u32;
        let completion_tokens = (cleaned.len() / 4).max(1) as u32;

        Ok(ChatCompletionResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model: request.model,
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content: Some(cleaned),
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
            return Err(ProviderError::Unavailable(
                "Perplexity Web: no cookie configured (set api_key to session token)".into()
            ));
        }

        let (system_msg, history) = parse_openai_messages(&request.messages);
        let current_msg = {
            let mut msg = String::new();
            if let Some(last) = request.messages.last() {
                if last.role == "user" {
                    msg = match &last.content {
                        Some(Content::Text(s)) => s.clone(),
                        Some(Content::Parts(parts)) => parts.iter()
                            .filter_map(|p| p.text.as_deref())
                            .collect::<Vec<_>>()
                            .join(" "),
                        None => String::new(),
                    };
                }
            }
            msg
        };

        let model_info = pplx_model_lookup(&request.model);
        let query = build_query(&system_msg, &history, &current_msg);
        let payload = build_pplx_body(&query, &model_info, None);

        let resp = self.send_stream_request(payload).await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status, body });
        }

        let cid = format!("chatcmpl-pplx-{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..12].to_string());
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

        // Parse SSE events from perplexity
        let mut full_answer = String::new();
        let mut seen_len: usize = 0;
        let mut data_lines: Vec<String> = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("data:") {
                data_lines.push(trimmed[5..].trim_start().to_string());
            } else if trimmed.is_empty() && !data_lines.is_empty() {
                let payload_str = data_lines.join("\n");
                data_lines.clear();
                if payload_str.trim() == "[DONE]" || payload_str.trim() == "end_of_stream" {
                    break;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&payload_str) {
                    if let Some(blocks) = val.get("blocks").and_then(|v| v.as_array()) {
                        for block in blocks {
                            if let Some(mb) = block.get("markdown_block") {
                                if let Some(arr) = mb.get("chunks").and_then(|v| v.as_array()) {
                                    if mb.get("progress").and_then(|v| v.as_str()) == Some("DONE") {
                                        full_answer = arr.iter()
                                            .filter_map(|c| c.as_str())
                                            .collect::<Vec<_>>()
                                            .join("");
                                    } else {
                                        let chunk_text: String = arr.iter()
                                            .filter_map(|c| c.as_str())
                                            .collect::<Vec<_>>()
                                            .join("");
                                        let cumulative = format!("{}{}", full_answer, chunk_text);
                                        if cumulative.len() > seen_len {
                                            let delta = cumulative[seen_len..].to_string();
                                            full_answer = cumulative;
                                            seen_len = full_answer.len();
                                            if !delta.is_empty() {
                                                chunks.push(Ok(ChatCompletionChunk {
                                                    id: cid.clone(),
                                                    object: "chat.completion.chunk".to_string(),
                                                    created,
                                                    model: model.clone(),
                                                    choices: vec![ChunkChoice {
                                                        index: 0,
                                                        delta: Some(Delta {
                                                            role: None,
                                                            content: Some(delta),
                                                            tool_calls: None,
                                                        }),
                                                        finish_reason: None,
                                                    }],
                                                    usage: None,
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Fallback: direct text field
                    if let Some(text_val) = val.get("text").and_then(|v| v.as_str()) {
                        let t = text_val.trim();
                        if t.len() > seen_len {
                            let delta = &t[seen_len..];
                            full_answer = t.to_string();
                            seen_len = t.len();
                            if !delta.is_empty() {
                                chunks.push(Ok(ChatCompletionChunk {
                                    id: cid.clone(),
                                    object: "chat.completion.chunk".to_string(),
                                    created,
                                    model: model.clone(),
                                    choices: vec![ChunkChoice {
                                        index: 0,
                                        delta: Some(Delta {
                                            role: None,
                                            content: Some(delta.to_string()),
                                            tool_calls: None,
                                        }),
                                        finish_reason: None,
                                    }],
                                    usage: None,
                                }));
                            }
                        }
                    }
                    if val.get("final").and_then(|v| v.as_bool()) == Some(true)
                        || val.get("status").and_then(|v| v.as_str()) == Some("COMPLETED")
                    {
                        break;
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
            owned_by: "perplexity-web".to_string(),
        }).collect();
        Ok(ModelListResponse { object: "list".to_string(), data })
    }
}
