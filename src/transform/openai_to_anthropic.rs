use crate::types::openai::{ChatCompletionRequest, Content};
use crate::types::anthropic::{MessagesRequest, AnthropicMessage, AnthropicContent, ContentBlock, SystemContent, SystemBlock};

/// Convert OpenAI chat completion request to Anthropic messages request
pub fn convert_chat_request(req: &ChatCompletionRequest) -> Result<MessagesRequest, String> {
    let mut system: Option<SystemContent> = None;
    let mut messages = Vec::new();

    for msg in &req.messages {
        if msg.role == "system" {
            let text = content_to_string(&msg.content).unwrap_or_default();
            system = Some(SystemContent::Blocks(vec![SystemBlock {
                block_type: "text".into(),
                text,
            }]));
            continue;
        }

        let content = match &msg.content {
            Some(Content::Text(t)) => AnthropicContent::Text(t.clone()),
            Some(Content::Parts(parts)) => {
                let blocks: Vec<ContentBlock> = parts.iter().map(|p| {
                    if p.part_type == "text" {
                        ContentBlock {
                            block_type: "text".into(),
                            text: p.text.clone(),
                            source: None,
                            id: None,
                            name: None,
                            input: None,
                            content: None,
                        }
                    } else if p.part_type == "image_url" {
                        if let Some(img) = &p.image_url {
                            let data = img.url.strip_prefix("data:").unwrap_or(&img.url);
                            let media_type = if data.starts_with("image/png") { "image/png" }
                                else if data.starts_with("image/jpeg") { "image/jpeg" }
                                else if data.starts_with("image/webp") { "image/webp" }
                                else { "image/png" };

                            let base64_data = if let Some(comma_pos) = data.find(',') {
                                data[comma_pos+1..].to_string()
                            } else {
                                data.to_string()
                            };

                            ContentBlock {
                                block_type: "image".into(),
                                text: None,
                                source: Some(crate::types::anthropic::ImageSource {
                                    source_type: "base64".into(),
                                    media_type: media_type.to_string(),
                                    data: base64_data,
                                }),
                                id: None,
                                name: None,
                                input: None,
                                content: None,
                            }
                        } else {
                            ContentBlock {
                                block_type: "text".into(),
                                text: Some("".into()),
                                source: None,
                                id: None,
                                name: None,
                                input: None,
                                content: None,
                            }
                        }
                    } else {
                        ContentBlock {
                            block_type: "text".into(),
                            text: Some("".into()),
                            source: None,
                            id: None,
                            name: None,
                            input: None,
                            content: None,
                        }
                    }
                }).collect();
                AnthropicContent::Blocks(blocks)
            }
            None => AnthropicContent::Text(String::new()),
        };

        messages.push(AnthropicMessage {
            role: msg.role.clone(),
            content,
        });
    }

    // Convert tools if present
    let tools = req.tools.as_ref().map(|tools| {
        tools.iter().map(|t| crate::types::anthropic::AnthropicTool {
            name: t.function.name.clone(),
            description: t.function.description.clone(),
            input_schema: Some(t.function.parameters.clone()),
        }).collect()
    });

    Ok(MessagesRequest {
        model: req.model.clone(),
        messages,
        stream: req.stream,
        max_tokens: req.max_tokens,
        system,
        temperature: req.temperature,
        top_p: req.top_p,
        top_k: None,
        stop_sequences: match &req.stop {
            Some(crate::types::openai::Stop::String(s)) => Some(vec![s.clone()]),
            Some(crate::types::openai::Stop::Array(v)) => Some(v.clone()),
            None => None,
        },
        tools,
        metadata: None,
    })
}

fn content_to_string(content: &Option<Content>) -> Option<String> {
    match content {
        Some(Content::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::openai::{Stop, Message};

    #[test]
    fn test_convert_simple_request() {
        let req = ChatCompletionRequest {
            model: "claude-sonnet-4-20250514".into(),
            messages: vec![
                Message { role: "system".into(), content: Some(Content::Text("Be helpful".into())), name: None, tool_calls: None, tool_call_id: None },
                Message { role: "user".into(), content: Some(Content::Text("hello".into())), name: None, tool_calls: None, tool_call_id: None },
            ],
            stream: None, temperature: None, max_tokens: None, top_p: None,
            frequency_penalty: None, presence_penalty: None, stop: None,
            tools: None, tool_choice: None, user: None, metadata: None,
        };
        let result = convert_chat_request(&req).unwrap();
        assert_eq!(result.model, "claude-sonnet-4-20250514");
        assert!(result.system.is_some());
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, "user");
    }

    #[test]
    fn test_convert_no_system_message() {
        let req = ChatCompletionRequest {
            model: "claude-3-haiku-20240307".into(),
            messages: vec![
                Message { role: "user".into(), content: Some(Content::Text("hi".into())), name: None, tool_calls: None, tool_call_id: None },
            ],
            stream: None, temperature: None, max_tokens: None, top_p: None,
            frequency_penalty: None, presence_penalty: None, stop: None,
            tools: None, tool_choice: None, user: None, metadata: None,
        };
        let result = convert_chat_request(&req).unwrap();
        assert!(result.system.is_none());
    }

    #[test]
    fn test_stop_sequences() {
        let mut req = ChatCompletionRequest::default();
        req.model = "test".into();
        req.stop = Some(Stop::Array(vec!["stop1".into(), "stop2".into()]));
        req.messages = vec![
            Message { role: "user".into(), content: Some(Content::Text("hi".into())), name: None, tool_calls: None, tool_call_id: None },
        ];
        let result = convert_chat_request(&req).unwrap();
        assert_eq!(result.stop_sequences.as_ref().unwrap().len(), 2);
    }
}
