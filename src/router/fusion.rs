use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::response::{IntoResponse, Json, Response};
use tokio::task::JoinSet;
use crate::config::settings::ComboConfig;
use crate::provider::{Provider, ProviderError, ProviderRegistry};
use crate::router::core::DispatchError;
use crate::tracker::RequestTracker;
use crate::types::openai::*;

/// Execute fusion strategy: parallel fan-out to all providers,
/// collect responses with quorum/grace, optionally judge-synthesize.
pub async fn execute_fusion(
    provider_names: Vec<String>,
    request: ChatCompletionRequest,
    registry: Arc<ProviderRegistry>,
    routes: &[crate::config::settings::RouteConfig],
    tracker: &RequestTracker,
    config: &ComboConfig,
    model: &str,
) -> Result<Response, DispatchError> {
    // Strip tools, force non-streaming
    let mut fusion_request = request.clone();
    fusion_request.stream = Some(false);
    fusion_request.tools = None;
    fusion_request.tool_choice = None;

    let hard_deadline = Instant::now() + Duration::from_millis(config.panel_hard_timeout_ms);
    let panel_hard_timeout = config.panel_hard_timeout_ms;
    let mut join_set = JoinSet::new();

    for pname in &provider_names {
        let req = fusion_request.clone();
        let pname = pname.clone();
        let reg = registry.clone();

        join_set.spawn(async move {
            let start = Instant::now();
            let provider = match reg.get(&pname) {
                Some(p) => p,
                None => return (pname, start.elapsed(), Err(ProviderError::Unavailable("not found".into()))),
            };

            let result = tokio::time::timeout(
                Duration::from_millis(panel_hard_timeout),
                provider.chat_completion(req),
            )
            .await
            .unwrap_or(Err(ProviderError::Http("fusion timeout".into())));

            (pname, start.elapsed(), result)
        });
    }

    if join_set.is_empty() {
        return Err(DispatchError::FusionError("No providers available for fusion".into()));
    }

    // Quorum/grace collection
    let min_panel = config.min_panel;
    let straggler_grace_ms = config.straggler_grace_ms;
    let model_owned = model.to_string();
    let mut responses: Vec<(String, Duration, ChatCompletionResponse)> = Vec::new();
    let mut grace_deadline: Option<Instant> = None;

    loop {
        let timeout = match grace_deadline {
            Some(gd) => {
                let now = Instant::now();
                if now >= gd { break; }
                let deadline = std::cmp::min(gd, hard_deadline);
                if now >= deadline { break; }
                deadline - now
            }
            None => {
                let now = Instant::now();
                if now >= hard_deadline { break; }
                hard_deadline - now
            }
        };

        match tokio::time::timeout(timeout, join_set.join_next()).await {
            Ok(Some(Ok((pname, latency, Ok(resp))))) => {
                responses.push((pname, latency, resp));

                if responses.len() >= min_panel && grace_deadline.is_none() {
                    grace_deadline = Some(Instant::now() + Duration::from_millis(straggler_grace_ms));
                }
            }
            Ok(Some(Ok((pname, _, Err(e))))) => {
                tracing::warn!(provider = %pname, error = %e, "Fusion panel provider failed");
                tracker.record_request(&pname, &model_owned, 0, false);
            }
            Ok(Some(Err(join_err))) => {
                tracing::warn!("Fusion join error: {}", join_err);
            }
            Ok(None) => break,
            Err(_) => {
                if let Some(gd) = grace_deadline {
                    if Instant::now() >= gd { break; }
                }
                break;
            }
        }
    }

    if responses.is_empty() {
        return Err(DispatchError::FusionError("All fusion providers failed".into()));
    }

    for (pname, latency, _) in &responses {
        tracker.record_request(pname, &model_owned, latency.as_millis() as u64, true);
    }

    // If judge model configured, synthesize
    if let Some(judge_model_name) = &config.judge_model {
        let original_prompt = extract_user_prompt(&request);
        let mut panel_text = String::new();
        for (i, (pname, _latency, resp)) in responses.iter().enumerate() {
            let content = resp.choices.first()
                .and_then(|c| c.message.content.as_deref())
                .unwrap_or("[no content]");
            panel_text.push_str(&format!("\n[Panel {} - {}]\n{}\n---\n", i + 1, pname, content));
        }

        let judge_request = ChatCompletionRequest {
            model: judge_model_name.clone(),
            messages: vec![
                Message {
                    role: "system".into(),
                    content: Some(Content::Text(
                        "You are a response synthesizer. Given a user request and multiple AI responses, synthesize the best combined answer. Be comprehensive and accurate.".into()
                    )),
                    name: None, tool_calls: None, tool_call_id: None,
                },
                Message {
                    role: "user".into(),
                    content: Some(Content::Text(format!(
                        "Original request:\n{}\n\nPanel responses (anonymized):\n{}", original_prompt, panel_text
                    ))),
                    name: None, tool_calls: None, tool_call_id: None,
                },
            ],
            stream: Some(false),
            ..Default::default()
        };

        if let Ok(judge_resp) = call_judge_async(&judge_request, &registry, routes).await {
            return Ok(Json(judge_resp).into_response());
        }
        tracing::warn!("Fusion judge failed, returning first panel response");
    }

    let (_, _, resp) = responses.remove(0);
    Ok(Json(resp).into_response())
}

fn extract_user_prompt(request: &ChatCompletionRequest) -> String {
    for msg in &request.messages {
        if msg.role == "user" {
            if let Some(content) = &msg.content {
                match content {
                    Content::Text(t) => return t.clone(),
                    Content::Parts(parts) => {
                        let texts: Vec<&str> = parts.iter()
                            .filter_map(|p| p.text.as_deref())
                            .collect();
                        if !texts.is_empty() {
                            return texts.join("\n");
                        }
                    }
                }
            }
        }
    }
    String::new()
}

pub fn resolve_single_provider<'a>(
    model: &str,
    registry: &'a ProviderRegistry,
    routes: &[crate::config::settings::RouteConfig],
) -> Result<&'a Box<dyn Provider>, ProviderError> {
    for route in routes {
        if route.model == model {
            if let Some(ref p) = route.provider {
                return registry.get(p).ok_or_else(|| ProviderError::Unavailable(format!("Judge provider '{}' not found", p)));
            }
            if let Some(ref ps) = route.providers {
                if let Some(first) = ps.first() {
                    return registry.get(first).ok_or_else(|| ProviderError::Unavailable(format!("Judge provider '{}' not found", first)));
                }
            }
        }
    }
    Err(ProviderError::Unavailable(format!("No route for judge model '{}'", model)))
}

pub async fn call_judge_async(
    request: &ChatCompletionRequest,
    registry: &ProviderRegistry,
    routes: &[crate::config::settings::RouteConfig],
) -> Result<ChatCompletionResponse, ()> {
    let provider = match resolve_single_provider(&request.model, registry, routes) {
        Ok(p) => p,
        Err(_) => return Err(()),
    };
    provider.chat_completion(request.clone()).await.map_err(|_| ())
}
