use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::router::core::RouteModel;
use crate::server::app::AppState;
use crate::types::openai::*;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/models", get(handle_list_models))
        .route("/openai/v1/chat/completions", post(handle_chat_completions))
        .route("/openai/v1/models", get(handle_list_models))
}

async fn handle_chat_completions(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, (StatusCode, Json<serde_json::Value>)> {
    let model = body.model.clone();
    let is_stream = body.stream.unwrap_or(false);

    // Resolve provider
    let route_result = state.registry.resolve(&model, &state.settings.routes);
    let provider = match route_result {
        Ok(p) => p,
        Err(e) => {
            let err = serde_json::json!({
                "error": {
                    "message": e,
                    "type": "invalid_request_error",
                    "param": "model",
                    "code": "model_not_found"
                }
            });
            return Err((StatusCode::NOT_FOUND, Json(err)));
        }
    };

    let provider_name = provider.name().to_string();

    if is_stream {
        return Err((StatusCode::NOT_IMPLEMENTED, Json(serde_json::json!({
            "error": {
                "message": "Streaming via direct endpoint not yet supported. Use non-streaming.",
                "type": "not_implemented",
                "param": null,
                "code": null
            }
        }))));
    }

    match provider.chat_completion(body).await {
        Ok(resp) => return Ok(Json(resp)),
        Err(_e) => {}
    }

    // Fallback on error
    let fallback_providers = state.registry.get_fallback_providers(&model, &state.settings.routes);
    for fb_provider in fallback_providers {
        tracing::warn!(primary = %provider_name, fallback = %fb_provider.name(), "Primary failed, trying fallback");
        match fb_provider.chat_completion(ChatCompletionRequest {
            model: model.clone(),
            ..Default::default()
        }).await {
            Ok(resp) => return Ok(Json(resp)),
            Err(_) => continue,
        }
    }

    let err = serde_json::json!({
        "error": {
            "message": format!("All providers failed"),
            "type": "upstream_error",
            "param": null,
            "code": "provider_error"
        }
    });
    Err((StatusCode::BAD_GATEWAY, Json(err)))
}

async fn handle_list_models(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ModelInfo>> {
    let mut models = Vec::new();
    for provider in state.registry.all() {
        for model_id in provider.models() {
            models.push(ModelInfo {
                id: model_id.clone(),
                object: "model".to_string(),
                created: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                owned_by: provider.name().to_string(),
            });
        }
    }
    Json(models)
}
