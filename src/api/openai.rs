use axum::{
    extract::State,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::server::app::AppState;
use crate::types::openai::*;

pub fn completions_routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/openai/v1/chat/completions", post(handle_chat_completions))
}

pub fn models_routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/models", get(handle_list_models))
        .route("/openai/v1/models", get(handle_list_models))
}

async fn handle_chat_completions(
    State(state): State<Arc<AppState>>,
    axum::extract::Json(body): axum::extract::Json<ChatCompletionRequest>,
) -> axum::response::Response {
    let is_stream = body.stream.unwrap_or(false);
    match state.engine.dispatch(body, is_stream, &state.tracker).await {
        Ok(resp) => resp,
        Err(e) => e.into_response(),
    }
}

async fn handle_list_models(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ModelInfo>> {
    let mut models = Vec::new();
    let reg = state.registry.load();
    for provider in reg.all() {
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
