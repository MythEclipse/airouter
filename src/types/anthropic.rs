use serde::{Deserialize, Serialize};

// ─── Messages Request ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<AnthropicMetadata>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ImageSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum SystemContent {
    Text(String),
    Blocks(Vec<SystemBlock>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SystemBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnthropicMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

// ─── Messages Response (Non-Streaming) ────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ResponseContentBlock>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ─── Streaming Events ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
}

// message_start
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageStartEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub message: MessagesResponse,
}

// content_block_start
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentBlockStartEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: u32,
    pub content_block: ResponseContentBlock,
}

// content_block_delta
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentBlockDeltaEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: u32,
    pub delta: DeltaBlock,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeltaBlock {
    #[serde(rename = "type")]
    pub delta_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partial_json: Option<String>,
}

// content_block_stop
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentBlockStopEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: u32,
}

// message_delta
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageDeltaEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub delta: MessageDelta,
    pub usage: Usage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

// message_stop
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MessageStopEvent {
    #[serde(rename = "type")]
    pub event_type: String,
}

// ping
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PingEvent {
    #[serde(rename = "type")]
    pub event_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_messages_request_serde() {
        let json = r#"{"model":"claude-sonnet-4-20250514","messages":[{"role":"user","content":"hello"}],"max_tokens":256}"#;
        let req: MessagesRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "claude-sonnet-4-20250514");
        assert_eq!(req.max_tokens, Some(256));
    }

    #[test]
    fn test_messages_response_serde() {
        let json = r#"{"id":"msg_1","type":"message","role":"assistant","content":[{"type":"text","text":"Hello!"}],"model":"claude-sonnet-4-20250514","stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":5}}"#;
        let resp: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "msg_1");
        assert_eq!(resp.content[0].text.as_deref(), Some("Hello!"));
    }

    #[test]
    fn test_stream_events_serde() {
        let json = r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":0}}}"#;
        let event: MessageStartEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "message_start");
        assert_eq!(event.message.id, "msg_1");
    }

    #[test]
    fn test_content_block_delta_serde() {
        let json = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event: ContentBlockDeltaEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "content_block_delta");
        assert_eq!(event.delta.delta_type, "text_delta");
        assert_eq!(event.delta.text.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_system_content() {
        let sys = SystemContent::Text("Be helpful.".into());
        match sys {
            SystemContent::Text(t) => assert_eq!(t, "Be helpful."),
            _ => panic!("expected Text variant"),
        }
    }

    #[test]
    fn test_anthropic_content_text() {
        let c = AnthropicContent::Text("hello".into());
        match c {
            AnthropicContent::Text(t) => assert_eq!(t, "hello"),
            _ => panic!("expected Text"),
        }
    }
}

// ─── Error Response ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AnthropicErrorResponse {
    pub error: AnthropicErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}
