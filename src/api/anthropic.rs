use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::post,
    Router,
};
use std::sync::Arc;
use crate::server::app::AppState;
use crate::types::anthropic::*;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/messages", post(handle_messages))
        .route("/anthropic/v1/messages", post(handle_messages))
}

async fn handle_messages(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MessagesRequest>,
) -> impl IntoResponse {
    let anthropic_provider = state.registry.all().find(|p| p.provider_type() == "anthropic");

    let provider = match anthropic_provider {
        Some(p) => p,
        None => {
            let err = AnthropicErrorResponse {
                error: AnthropicErrorDetail {
                    error_type: "invalid_request_error".into(),
                    message: "No Anthropic provider configured".into(),
                },
            };
            return Err((StatusCode::NOT_FOUND, Json(err)));
        }
    };

    // Convert to OpenAI internal format
    let openai_req = crate::transform::anthropic_to_openai::convert_messages_request(&body);
    let _is_stream = body.stream.unwrap_or(false);

    // Non-streaming call
    match provider.chat_completion(openai_req).await {
        Ok(openai_resp) => {
            // Convert OpenAI response back to Anthropic format
            let anthro_resp = serde_json::json!({
                "id": openai_resp.id,
                "type": "message",
                "role": "assistant",
                "content": [
                    {
                        "type": "text",
                        "text": openai_resp.choices.first().and_then(|c| c.message.content.as_deref()).unwrap_or("")
                    }
                ],
                "model": openai_resp.model,
                "stop_reason": openai_resp.choices.first().and_then(|c| c.finish_reason.as_deref()).unwrap_or("end_turn"),
                "stop_sequence": null,
                "usage": {
                    "input_tokens": openai_resp.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0),
                    "output_tokens": openai_resp.usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0)
                }
            });
            Ok(Json(anthro_resp))
        }
        Err(e) => {
            let err = AnthropicErrorResponse {
                error: AnthropicErrorDetail {
                    error_type: "api_error".into(),
                    message: e.to_string(),
                },
            };
            Err((StatusCode::BAD_GATEWAY, Json(err)))
        }
    }
}
