use crate::types::openai::*;
use crate::types::anthropic::{self, MessagesRequest};
use std::time::{SystemTime, UNIX_EPOCH};

/// Convert Anthropic messages request to OpenAI chat completion format
pub fn convert_messages_request(req: &MessagesRequest) -> ChatCompletionRequest {
    let mut messages = Vec::new();

    // If system prompt present, add as system message
    if let Some(system) = &req.system {
        match system {
            anthropic::SystemContent::Text(t) => {
                messages.push(Message {
                    role: "system".into(),
                    content: Some(Content::Text(t.clone())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            anthropic::SystemContent::Blocks(blocks) => {
                let text = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("
");
                messages.push(Message {
                    role: "system".into(),
                    content: Some(Content::Text(text)),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        }
    }

    for anthro_msg in &req.messages {
        let content = match &anthro_msg.content {
            anthropic::AnthropicContent::Text(t) => Content::Text(t.clone()),
            anthropic::AnthropicContent::Blocks(blocks) => {
                let parts: Vec<ContentPart> = blocks.iter().map(|b| {
                    if b.block_type == "text" {
                        ContentPart {
                            part_type: "text".into(),
                            text: b.text.clone(),
                            image_url: None,
                        }
                    } else {
                        ContentPart {
                            part_type: "text".into(),
                            text: Some("".into()),
                            image_url: None,
                        }
                    }
                }).collect();
                Content::Parts(parts)
            }
        };

        messages.push(Message {
            role: anthro_msg.role.clone(),
            content: Some(content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    ChatCompletionRequest {
        model: req.model.clone(),
        messages,
        stream: req.stream,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        top_p: req.top_p,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        tools: None,
        tool_choice: None,
        user: None,
        metadata: None,
    }
}

/// Convert Anthropic MessagesResponse to OpenAI ChatCompletionResponse
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::anthropic::*;

    #[test]
    fn test_convert_messages_request_with_system() {
        let req = MessagesRequest {
            model: "gpt-4o".into(),
            messages: vec![
                AnthropicMessage { role: "user".into(), content: AnthropicContent::Text("hello".into()) },
            ],
            stream: None, max_tokens: None,
            system: Some(SystemContent::Text("Be helpful".into())),
            temperature: None, top_p: None, top_k: None,
            stop_sequences: None, tools: None, metadata: None,
        };
        let openai = convert_messages_request(&req);
        assert_eq!(openai.model, "gpt-4o");
        assert_eq!(openai.messages.len(), 2); // system + user
        assert_eq!(openai.messages[0].role, "system");
    }

    #[test]
    fn test_convert_messages_request_no_system() {
        let req = MessagesRequest {
            model: "gpt-4o".into(),
            messages: vec![
                AnthropicMessage { role: "user".into(), content: AnthropicContent::Text("hi".into()) },
            ],
            stream: None, max_tokens: None, system: None,
            temperature: None, top_p: None, top_k: None,
            stop_sequences: None, tools: None, metadata: None,
        };
        let openai = convert_messages_request(&req);
        assert_eq!(openai.messages.len(), 1);
    }

    #[test]
    fn test_convert_anthropic_response() {
        let resp = MessagesResponse {
            id: "msg_1".into(),
            response_type: "message".into(),
            role: "assistant".into(),
            content: vec![ResponseContentBlock {
                block_type: "text".into(),
                text: Some("Hello!".into()),
                id: None, name: None, input: None, content: None,
            }],
            model: "claude-sonnet-4-20250514".into(),
            stop_reason: Some("end_turn".into()),
            stop_sequence: None,
            usage: anthropic::Usage { input_tokens: 10, output_tokens: 5 },
        };
        let result = convert_anthropic_response(&resp, "gpt-4o").unwrap();
        assert_eq!(result.choices[0].message.content.as_deref(), Some("Hello!"));
        assert_eq!(result.usage.as_ref().unwrap().total_tokens, 15);
    }

    #[test]
    fn test_convert_stop_reason_tool_use() {
        let resp = MessagesResponse {
            id: "msg_2".into(), response_type: "message".into(),
            role: "assistant".into(), content: vec![], model: "claude-3-5-sonnet".into(),
            stop_reason: Some("tool_use".into()), stop_sequence: None,
            usage: anthropic::Usage { input_tokens: 5, output_tokens: 3 },
        };
        let result = convert_anthropic_response(&resp, "gpt-4o").unwrap();
        assert_eq!(result.choices[0].finish_reason.as_deref(), Some("tool_calls"));
    }
}

pub fn convert_anthropic_response(
    resp: &anthropic::MessagesResponse,
    model: &str,
) -> Result<ChatCompletionResponse, String> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let mut content = String::new();

    for block in &resp.content {
        if block.block_type == "text" {
            if let Some(text) = &block.text {
                content.push_str(text);
            }
        }
    }

    let finish_reason = resp.stop_reason.as_deref().map(|s| match s {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        other => other,
    }).map(|s| s.to_string());

    Ok(ChatCompletionResponse {
        id: resp.id.clone(),
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
            prompt_tokens: resp.usage.input_tokens,
            completion_tokens: resp.usage.output_tokens,
            total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
        }),
    })
}
