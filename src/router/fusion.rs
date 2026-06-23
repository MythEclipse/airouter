use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::response::{IntoResponse, Json, Response};
use tokio::task::JoinSet;
use crate::config::settings::ComboConfig;
use crate::provider::{Provider, ProviderError, ProviderRegistry};
use crate::router::core::DispatchError;
use crate::tracker::RequestTracker;
use arc_swap::ArcSwap;
use metrics::{counter, histogram};
use crate::types::openai::*;

/// Execute fusion strategy: parallel fan-out to all providers,
/// collect responses with quorum/grace, optionally judge-synthesize.
pub async fn execute_fusion(
    provider_names: Vec<String>,
    request: ChatCompletionRequest,
    registry: &Arc<ArcSwap<ProviderRegistry>>,
    tracker: &RequestTracker,
    config: &ComboConfig,
    redis: &redis::aio::ConnectionManager,
    model: &str,
) -> Result<Response, DispatchError> {
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
        let reg = registry.load().clone();

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
                counter!("airouter_provider_requests_total",
                    "provider" => pname.clone(), "model" => model_owned.clone(), "status" => "success").increment(1);
                histogram!("airouter_provider_latency_ms",
                    "provider" => pname.clone(), "model" => model_owned.clone()).record(latency.as_millis() as f64);
                responses.push((pname, latency, resp));
                if responses.len() >= min_panel && grace_deadline.is_none() {
                    grace_deadline = Some(Instant::now() + Duration::from_millis(straggler_grace_ms));
                }
            }
            Ok(Some(Ok((pname, latency, Err(e))))) => {
                let elapsed_ms = latency.as_millis() as f64;
                let class = e.error_class();
                counter!("airouter_provider_requests_total",
                    "provider" => pname.clone(), "model" => model_owned.clone(), "status" => "error").increment(1);
                histogram!("airouter_provider_latency_ms",
                    "provider" => pname.clone(), "model" => model_owned.clone()).record(elapsed_ms);
                counter!("airouter_provider_errors_total",
                    "provider" => pname.clone(), "error_class" => class.as_label_str()).increment(1);
                let error_kind = format!("{:?}", class);
                let error_detail = e.to_string();
                tracing::warn!(
                    provider = %pname,
                    model = %model_owned,
                    latency_ms = latency.as_millis() as u64,
                    error = %e,
                    error_kind = %error_kind,
                    error_detail = %error_detail,
                    "Fusion panel provider failed",
                );
                tracker.record_request(redis, &pname, &model_owned, 0, false).await;
            }
            Ok(Some(Err(join_err))) => {
                tracing::warn!(
                    model = %model_owned,
                    latency_ms = 0u64,
                    error_kind = "JoinError",
                    error_detail = %join_err,
                    "Fusion join error",
                );
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    if responses.is_empty() {
        return Err(DispatchError::FusionError("All fusion providers failed".into()));
    }

    for (pname, latency, _) in &responses {
        tracker.record_request(redis, pname, &model_owned, latency.as_millis() as u64, true).await;
    }

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
                        "You are a response synthesizer...".into()
                    )),
                    name: None, tool_calls: None, tool_call_id: None,
                },
                Message {
                    role: "user".into(),
                    content: Some(Content::Text(format!(
                        "Original request:\n{}\n\nPanel responses:\n{}", original_prompt, panel_text
                    ))),
                    name: None, tool_calls: None, tool_call_id: None,
                },
            ],
            stream: Some(false),
            ..Default::default()
        };

        if let Ok(judge_resp) = call_judge_async(&judge_request, registry).await {
            return Ok(Json(judge_resp).into_response());
        }
        tracing::warn!(
            error_kind = "FusionJudgeFailed",
            error_detail = "Judge model call failed, falling back to first panel response",
            "Fusion judge failed, returning first panel response",
        );
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

pub async fn call_judge_async(
    request: &ChatCompletionRequest,
    registry: &Arc<ArcSwap<ProviderRegistry>>,
) -> Result<ChatCompletionResponse, ()> {
    let reg = registry.load();
    let provider = reg.get(&request.model)
        .ok_or_else(|| ())?;
    provider.chat_completion(request.clone()).await.map_err(|_| ())
}
