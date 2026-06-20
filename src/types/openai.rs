use serde::{Deserialize, Serialize};

// ─── Chat Completion Request ──────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Default for ChatCompletionRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            messages: Vec::new(),
            stream: None,
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            tools: None,
            tool_choice: None,
            user: None,
            metadata: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum Stop {
    String(String),
    Array(Vec<String>),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// ─── Chat Completion Response (Non-Streaming) ─────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResponseMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ─── Streaming Chat Completion Chunk ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkChoice {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<Delta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

// ─── Error Response ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OpenAIErrorResponse {
    pub error: OpenAIErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct OpenAIErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}

// ─── Models List ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_completion_request_serde() {
        let json = r#"{"model":"gpt-4o","messages":[{"role":"user","content":"hello"}],"stream":false}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model, "gpt-4o");
        assert!(!req.messages.is_empty());
        assert_eq!(req.messages[0].role, "user");
    }

    #[test]
    fn test_chat_completion_response_serde() {
        let json = r#"{"id":"chat-1","object":"chat.completion","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"message":{"role":"assistant","content":"Hello!"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "chat-1");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content.as_deref(), Some("Hello!"));
    }

    #[test]
    fn test_chat_completion_chunk_serde() {
        let json = r#"{"id":"chunk-1","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(json).unwrap();
        assert_eq!(chunk.id, "chunk-1");
        assert_eq!(chunk.choices[0].delta.as_ref().unwrap().content.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_model_list_serde() {
        let resp = ModelListResponse {
            object: "list".into(),
            data: vec![ModelInfo { id: "gpt-4o".into(), object: "model".into(), created: 0, owned_by: "openai".into() }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("gpt-4o"));
    }

    #[test]
    fn test_chat_completion_default() {
        let req = ChatCompletionRequest::default();
        assert_eq!(req.model, "");
        assert!(req.messages.is_empty());
        assert!(req.stream.is_none());
    }

    #[test]
    fn test_message_with_tool_calls() {
        let json = r#"{"role":"assistant","content":"Let me check","tool_calls":[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{\"city\":\"Jakarta\"}"}}]}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.as_ref().unwrap()[0].function.name, "get_weather");
    }
}
