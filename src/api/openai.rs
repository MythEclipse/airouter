use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse, Json, Response,
    },
    routing::{get, post},
    Router,
};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_stream::StreamExt;
use crate::router::balancer::LoadBalancer;
use crate::router::core::RouteModel;
use crate::server::app::AppState;
use crate::types::openai::*;

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/models", get(handle_list_models))
        .route("/openai/v1/chat/completions", post(handle_chat_completions))
        .route("/openai/v1/models", get(handle_list_models))
}

fn model_error(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({
        "error": { "message": msg, "type": "invalid_request_error", "param": "model", "code": "model_not_found" }
    })))
}

fn upstream_error(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
        "error": { "message": msg, "type": "upstream_error", "param": null, "code": "provider_error" }
    })))
}

/// Resolve provider names for a model, using load balancer if multiple.
/// Returns Vec<String> so there are no lifetime borrow conflicts.
fn resolve_provider_names(
    state: &AppState,
    model: &str,
    balancer: &LoadBalancer,
) -> Result<Vec<String>, (StatusCode, Json<serde_json::Value>)> {
    let route_result = state.registry.resolve(model, &state.settings.routes);
    let primary = match route_result {
        Ok(p) => p,
        Err(e) => return Err(model_error(&e)),
    };

    let mut names: Vec<String> = vec![primary.name().to_string()];

    // Get fallbacks
    let fallbacks = state.registry.get_fallback_providers(model, &state.settings.routes);
    for fb in fallbacks {
        if !names.iter().any(|n| n == fb.name()) {
            names.push(fb.name().to_string());
        }
    }

    // Apply load balancer round-robin
    if let Some(selected) = balancer.select_by_name(&names) {
        // Reorder: selected first, then others
        let mut reordered = vec![selected.clone()];
        for n in &names {
            if *n != selected {
                reordered.push(n.clone());
            }
        }
        Ok(reordered)
    } else {
        Ok(names)
    }
}

async fn handle_chat_completions(
    State(state): State<Arc<AppState>>,
    axum::extract::Json(body): axum::extract::Json<ChatCompletionRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let model = body.model.clone();
    let is_stream = body.stream.unwrap_or(false);
    let start = SystemTime::now();

    let provider_names = resolve_provider_names(&state, &model, &state.balancer)?;

    if is_stream {
        return handle_streaming(state, body, provider_names, start).await;
    }

    handle_non_streaming(state, body, provider_names, start).await
}

async fn handle_non_streaming(
    state: Arc<AppState>,
    body: ChatCompletionRequest,
    provider_names: Vec<String>,
    start: SystemTime,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let model = body.model.clone();
    let mut last_error = String::new();

    for (i, pname) in provider_names.iter().enumerate() {
        let provider = match state.registry.get(pname) {
            Some(p) => p,
            None => continue,
        };

        if i > 0 && state.balancer.is_on_cooldown(pname) {
            tracing::debug!(provider = %pname, "Skipping on cooldown");
            continue;
        }

        match provider.chat_completion(body.clone()).await {
            Ok(resp) => {
                let elapsed = start.elapsed().unwrap_or_default().as_millis();
                tracing::info!(
                    provider = %pname, model = %model, latency_ms = elapsed,
                    "Non-streaming completion succeeded"
                );
                state.tracker.record_request(pname, &model, elapsed as u64, true);
                state.balancer.clear_cooldown(pname);
                return Ok(Json(resp).into_response());
            }
            Err(e) => {
                let elapsed = start.elapsed().unwrap_or_default().as_millis();
                tracing::warn!(
                    provider = %pname, model = %model, latency_ms = elapsed, error = %e,
                    "Provider failed"
                );
                state.tracker.record_request(pname, &model, elapsed as u64, false);
                state.balancer.mark_cooldown(pname);
                last_error = e.to_string();
                if !e.is_retryable() {
                    break;
                }
            }
        }
    }

    Err(upstream_error(&format!("All providers failed: {}", last_error)))
}

async fn handle_streaming(
    state: Arc<AppState>,
    body: ChatCompletionRequest,
    provider_names: Vec<String>,
    start: SystemTime,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let model = body.model.clone();
    let mut last_error = String::new();

    for (i, pname) in provider_names.iter().enumerate() {
        let provider = match state.registry.get(pname) {
            Some(p) => p,
            None => continue,
        };

        if i > 0 && state.balancer.is_on_cooldown(pname) {
            continue;
        }

        match provider.chat_completion_stream(body.clone()).await {
            Ok(provider_stream) => {
                let elapsed = start.elapsed().unwrap_or_default().as_millis();
                state.tracker.record_request(pname, &model, elapsed as u64, true);
                state.balancer.clear_cooldown(pname);

                let chunk_stream = provider_stream.map(|chunk_result| {
                    match chunk_result {
                        Ok(chunk) => {
                            let data = serde_json::to_string(&chunk).unwrap_or_default();
                            Ok::<Event, Infallible>(Event::default().data(data))
                        }
                        Err(_) => {
                            Ok::<Event, Infallible>(Event::default().data("[DONE]"))
                        }
                    }
                });

                let done_stream = futures::stream::once(async {
                    Ok::<Event, Infallible>(Event::default().data("[DONE]"))
                });
                let full_stream = chunk_stream.chain(done_stream);

                let sse = Sse::new(full_stream)
                    .keep_alive(axum::response::sse::KeepAlive::new()
                        .interval(std::time::Duration::from_secs(15))
                        .text("keep-alive"));

                return Ok(sse.into_response());
            }
            Err(e) => {
                let elapsed = start.elapsed().unwrap_or_default().as_millis();
                tracing::warn!(
                    provider = %pname, model = %model, error = %e,
                    "Stream provider failed"
                );
                state.tracker.record_request(pname, &model, elapsed as u64, false);
                state.balancer.mark_cooldown(pname);
                last_error = e.to_string();
                if !e.is_retryable() {
                    break;
                }
            }
        }
    }

    Err(upstream_error(&format!("All streaming providers failed: {}", last_error)))
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
