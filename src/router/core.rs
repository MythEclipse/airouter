use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::http::StatusCode;
use axum::response::{
    sse::{Event, Sse},
    IntoResponse, Json, Response,
};
use std::convert::Infallible;
use tokio_stream::StreamExt;
use arc_swap::ArcSwap;
use crate::config::settings::{RouteConfig, Settings, StrategyKind};
use crate::provider::{ErrorClass, ProviderRegistry};
use crate::router::balancer::LoadBalancer;
use crate::tracker::RequestTracker;
use crate::types::openai::*;

// ─── Dispatch Error ──────────────────────────────────────────────

#[derive(Debug)]
pub enum DispatchError {
    ModelNotFound(String),
    AllProvidersFailed(String),
    FusionError(String),
}

impl IntoResponse for DispatchError {
    fn into_response(self) -> Response {
        let (status, msg, code) = match &self {
            DispatchError::ModelNotFound(m) => (StatusCode::NOT_FOUND, m.clone(), "model_not_found"),
            DispatchError::AllProvidersFailed(m) => (StatusCode::BAD_GATEWAY, m.clone(), "provider_error"),
            DispatchError::FusionError(m) => (StatusCode::BAD_GATEWAY, m.clone(), "fusion_error"),
        };
        let err_kind = match &self {
            DispatchError::ModelNotFound(_) => "invalid_request_error",
            _ => "upstream_error",
        };
        (
            status,
            Json(serde_json::json!({
                "error": { "message": msg, "type": err_kind, "param": null, "code": code }
            })),
        ).into_response()
    }
}

// ─── RouteEngine ─────────────────────────────────────────────────

pub struct RouteEngine {
    registry: Arc<ArcSwap<ProviderRegistry>>,
    config: Arc<ArcSwap<Settings>>,
    balancer: Arc<LoadBalancer>,
    redis: redis::aio::ConnectionManager,
}

impl RouteEngine {
    pub fn new(
        registry: Arc<ArcSwap<ProviderRegistry>>,
        config: Arc<ArcSwap<Settings>>,
        balancer: Arc<LoadBalancer>,
        redis: redis::aio::ConnectionManager,
    ) -> Self {
        Self { registry, config, balancer, redis }
    }

    /// Find a RouteConfig for a model (exact match or wildcard *)
    fn find_route(&self, model: &str) -> Option<RouteConfig> {
        let settings = self.config.load();
        for route in &settings.routes {
            if route.model == model {
                return Some(route.clone());
            }
        }
        for route in &settings.routes {
            if route.model == "*" {
                return Some(route.clone());
            }
        }
        None
    }

    /// Detect capabilities needed from request messages
    pub fn detect_capabilities(request: &ChatCompletionRequest) -> Vec<&'static str> {
        let mut caps = Vec::new();
        for msg in &request.messages {
            if let Some(content) = &msg.content {
                match content {
                    Content::Parts(parts) => {
                        for part in parts {
                            match part.part_type.as_str() {
                                "image_url" if !caps.contains(&"vision") => caps.push("vision"),
                                "input_audio" if !caps.contains(&"audio") => caps.push("audio"),
                                _ => {}
                            }
                        }
                    }
                    Content::Text(_) => {}
                }
            }
        }
        caps
    }

    /// Reorder providers so those with matching capabilities come first
    pub fn reorder_by_capability(
        providers: Vec<String>,
        capabilities: &[&str],
        registry: &ProviderRegistry,
    ) -> Vec<String> {
        if capabilities.is_empty() {
            return providers;
        }
        let mut matched = Vec::new();
        let mut unmatched = Vec::new();
        for p in providers {
            let has_any = capabilities.iter().any(|c| registry.has_capability(&p, c));
            if has_any {
                matched.push(p);
            } else {
                unmatched.push(p);
            }
        }
        matched.extend(unmatched);
        matched
    }

    /// Extract ordered provider names for a model, with capability reordering
    async fn get_provider_names(&self, model: &str, capabilities: &[&str]) -> Result<Vec<String>, DispatchError> {
        let route = self.find_route(model).ok_or_else(|| {
            DispatchError::ModelNotFound(format!("No route found for model '{}'", model))
        })?;

        let mut names: Vec<String> = Vec::new();
        if let Some(ref p) = route.provider {
            names.push(p.clone());
        }
        if let Some(ref ps) = route.providers {
            for p in ps {
                if !names.contains(p) {
                    names.push(p.clone());
                }
            }
        }

        if names.is_empty() {
            return Err(DispatchError::ModelNotFound(format!("No providers for model '{}'", model)));
        }

        // Capability reorder
        let registry = self.registry.load();
        if !capabilities.is_empty() {
            names = Self::reorder_by_capability(names, capabilities, &registry);
        }
        drop(registry);

        // Load balancer selection: reorder so selected is first
        if let Some(selected) = self.balancer.select_by_name(&names).await {
            let mut reordered = vec![selected.clone()];
            for n in names {
                if n != selected {
                    reordered.push(n);
                }
            }
            Ok(reordered)
        } else {
            Ok(names)
        }
    }

    // ─── Main Dispatch ───────────────────────────────────────

    pub async fn dispatch(
        &self,
        mut request: ChatCompletionRequest,
        is_stream: bool,
        tracker: &RequestTracker,
    ) -> Result<Response, DispatchError> {
        // Inject default_max_tokens from server config if request doesn't set it
        if request.max_tokens.is_none() {
            let settings = self.config.load();
            if let Some(mt) = settings.server.default_max_tokens {
                request.max_tokens = Some(mt);
            }
            drop(settings);
        }

        let model = request.model.clone();
        let capabilities = Self::detect_capabilities(&request);
        let provider_names = self.get_provider_names(&model, &capabilities).await?;
        let route = self.find_route(&model).ok_or_else(|| {
            DispatchError::ModelNotFound(format!("No route for model '{}'", model))
        })?;

        let settings = self.config.load();
        let strategy = route.effective_strategy(settings.default_strategy.as_ref());
        drop(settings);

        match strategy {
            StrategyKind::Fusion => {
                crate::router::fusion::execute_fusion(
                    provider_names, request,
                    &self.registry,
                    tracker,
                    route.combo.as_ref().unwrap_or(&Default::default()),
                    &self.redis, &model,
                ).await
            }
            StrategyKind::RoundRobin => {
                self.execute_round_robin(provider_names, request, &route, is_stream, tracker, &model).await
            }
            StrategyKind::Fallback | StrategyKind::Single => {
                self.execute_sequential(provider_names, request, is_stream, tracker, &model).await
            }
        }
    }

    // ─── Sequential Execution (Single / Fallback) ────────────

    async fn execute_sequential(
        &self,
        provider_names: Vec<String>,
        request: ChatCompletionRequest,
        is_stream: bool,
        tracker: &RequestTracker,
        model: &str,
    ) -> Result<Response, DispatchError> {
        let start = Instant::now();
        let mut last_error = String::new();

        for (i, pname) in provider_names.iter().enumerate() {
            let registry = self.registry.load();
            let provider = match registry.get(pname) {
                Some(p) => p,
                None => continue,
            };

            if i > 0 && self.balancer.is_on_cooldown(pname).await {
                tracing::debug!(provider = %pname, "Skipping on cooldown");
                continue;
            }

            if is_stream {
                match provider.chat_completion_stream(request.clone()).await {
                    Ok(provider_stream) => {
                        let elapsed = start.elapsed().as_millis();
                        tracker.record_request(&self.redis, pname, model, elapsed as u64, true).await;
                        self.balancer.clear_cooldown(pname).await;

                        let chunk_stream = provider_stream.map(|chunk_result| {
                            match chunk_result {
                                Ok(chunk) => {
                                    let data = serde_json::to_string(&chunk).unwrap_or_default();
                                    Ok::<Event, Infallible>(Event::default().data(data))
                                }
                                Err(_) => Ok::<Event, Infallible>(Event::default().data("[DONE]"))
                            }
                        });
                        let done_stream = futures::stream::once(async {
                            Ok::<Event, Infallible>(Event::default().data("[DONE]"))
                        });
                        let full_stream = chunk_stream.chain(done_stream);
                        let sse = Sse::new(full_stream)
                            .keep_alive(axum::response::sse::KeepAlive::new()
                                .interval(Duration::from_secs(15))
                                .text("keep-alive"));
                        return Ok(sse.into_response());
                    }
                    Err(e) => {
                        let elapsed = start.elapsed().as_millis();
                        let class = e.error_class();
                        tracing::warn!(
                            provider = %pname,
                            model = %model,
                            error = %e,
                            error_kind = %format!("{:?}", class),
                            error_detail = %e,
                            latency_ms = elapsed as u64,
                            "Stream provider failed",
                        );
                        tracker.record_request(&self.redis, pname, model, elapsed as u64, false).await;
                        self.balancer.mark_cooldown_with_class(pname, class).await;
                        last_error = e.to_string();
                        if !e.is_retryable() || class == ErrorClass::BadRequest {
                            break;
                        }
                    }
                }
            } else {
                match provider.chat_completion(request.clone()).await {
                    Ok(resp) => {
                        let elapsed = start.elapsed().as_millis();
                        tracker.record_request(&self.redis, pname, model, elapsed as u64, true).await;
                        self.balancer.clear_cooldown(pname).await;
                        return Ok(Json(resp).into_response());
                    }
                    Err(e) => {
                        let elapsed = start.elapsed().as_millis();
                        let class = e.error_class();
                        tracing::warn!(
                            provider = %pname,
                            model = %model,
                            error = %e,
                            error_kind = %format!("{:?}", class),
                            error_detail = %e,
                            latency_ms = elapsed as u64,
                            "Provider failed",
                        );
                        tracker.record_request(&self.redis, pname, model, elapsed as u64, false).await;
                        self.balancer.mark_cooldown_with_class(pname, class).await;
                        last_error = e.to_string();
                        if !e.is_retryable() || class == ErrorClass::BadRequest {
                            break;
                        }
                    }
                }
            }
        }

        tracing::error!(
            error_kind = "AllProvidersFailed",
            error_detail = %last_error,
            latency_ms = %start.elapsed().as_millis(),
            "All providers failed",
        );
        Err(DispatchError::AllProvidersFailed(format!("All providers failed: {}", last_error)))
    }

    // ─── Round-Robin Execution ──────────────────────────────

    async fn execute_round_robin(
        &self,
        provider_names: Vec<String>,
        request: ChatCompletionRequest,
        route: &RouteConfig,
        is_stream: bool,
        tracker: &RequestTracker,
        model: &str,
    ) -> Result<Response, DispatchError> {
        let sticky_limit = route.combo.as_ref().and_then(|c| c.sticky_limit);
        let rotated = self.rotate_providers(model, provider_names, sticky_limit).await;
        self.execute_sequential(rotated, request, is_stream, tracker, model).await
    }

    /// Redis-based round-robin rotation with optional sticky limit
    async fn rotate_providers(
        &self,
        model: &str,
        providers: Vec<String>,
        sticky_limit: Option<usize>,
    ) -> Vec<String> {
        let index_key = format!("rotation_index:{}", model);
        let mut conn = self.redis.clone();
        let idx_raw: Result<i64, _> = redis::cmd("INCR")
            .arg(&index_key)
            .query_async(&mut conn).await;
        let len = if providers.is_empty() { 1 } else { providers.len() };
        let idx = (idx_raw.unwrap_or(0) as usize) % len;
        let selected = providers[idx].clone();

        if let Some(limit) = sticky_limit {
            let count_key = format!("rotation_count:{}:{}", model, selected);
            let mut conn2 = self.redis.clone();
            let count: i64 = redis::cmd("GET")
                .arg(&count_key)
                .query_async(&mut conn2).await.unwrap_or(0);

            if count < limit as i64 {
                let mut conn3 = self.redis.clone();
                let _: Result<(), _> = redis::cmd("INCR")
                    .arg(&count_key)
                    .query_async(&mut conn3).await;

                let (mut matched, others): (Vec<_>, Vec<_>) = providers.into_iter()
                    .partition(|p| *p == selected);
                matched.extend(others);
                return matched;
            }
            // Exceeded limit, reset count
            let mut conn4 = self.redis.clone();
            let _: Result<(), _> = redis::cmd("SET")
                .arg(&count_key).arg("1")
                .query_async(&mut conn4).await;
        }

        let (mut matched, others): (Vec<_>, Vec<_>) = providers.into_iter()
            .partition(|p| *p == selected);
        matched.extend(others);
        matched
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::RouteConfig;
    use crate::types::openai::*;
    use crate::provider::ProviderRegistry;

    fn make_routes() -> Vec<RouteConfig> {
        vec![
            RouteConfig { model: "gpt-4o".into(), strategy: StrategyKind::Single, provider: Some("openai".into()), providers: None, combo: None },
            RouteConfig { model: "*".into(), strategy: StrategyKind::Fallback, provider: None, providers: Some(vec!["openai".into(), "groq".into()]), combo: None },
        ]
    }

    #[test]
    fn test_resolve_exact_match() {
        let routes = make_routes();
        assert!(routes[0].model == "gpt-4o");
        assert_eq!(routes[0].provider.as_deref(), Some("openai"));
    }

    #[test]
    fn test_resolve_wildcard_match() {
        let routes = make_routes();
        assert!(routes[1].model == "*");
        assert_eq!(routes[1].providers.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_no_match_not_found() {
        let routes = make_routes();
        let has = routes.iter().any(|r| r.model == "unknown-model" || r.model == "*");
        assert!(has);
    }

    #[test]
    fn test_route_strategies() {
        let single = RouteConfig { model: "a".into(), strategy: StrategyKind::Single, provider: Some("p1".into()), providers: None, combo: None };
        let fallback = RouteConfig { model: "b".into(), strategy: StrategyKind::Fallback, provider: None, providers: Some(vec!["p1".into(), "p2".into()]), combo: None };
        assert_eq!(single.strategy, StrategyKind::Single);
        assert_eq!(fallback.strategy, StrategyKind::Fallback);
        assert_eq!(fallback.providers.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_detect_capabilities_text_only() {
        let req = ChatCompletionRequest {
            model: "test".into(),
            messages: vec![Message {
                role: "user".into(),
                content: Some(Content::Text("hello".into())),
                name: None, tool_calls: None, tool_call_id: None,
            }],
            ..Default::default()
        };
        let caps = RouteEngine::detect_capabilities(&req);
        assert!(caps.is_empty());
    }

    #[test]
    fn test_detect_capabilities_with_image() {
        let req = ChatCompletionRequest {
            model: "test".into(),
            messages: vec![Message {
                role: "user".into(),
                content: Some(Content::Parts(vec![ContentPart {
                    part_type: "image_url".into(),
                    text: None,
                    image_url: Some(ImageUrl { url: "data:...".into(), detail: None }),
                }])),
                name: None, tool_calls: None, tool_call_id: None,
            }],
            ..Default::default()
        };
        let caps = RouteEngine::detect_capabilities(&req);
        assert_eq!(caps, vec!["vision"]);
    }

    #[test]
    fn test_reorder_by_capability_none_needed() {
        let providers = vec!["a".into(), "b".into()];
        let registry = ProviderRegistry::new();
        let result = RouteEngine::reorder_by_capability(providers.clone(), &[], &registry);
        assert_eq!(result, providers);
    }

    #[test]
    fn test_reorder_by_capability_sort() {
        let mut registry = ProviderRegistry::new();
        registry.capabilities.insert("vision-provider".into(), vec!["vision".into()]);
        registry.capabilities.insert("text-provider".into(), vec![]);

        let providers = vec!["text-provider".into(), "vision-provider".into()];
        let result = RouteEngine::reorder_by_capability(providers, &["vision"], &registry);
        assert_eq!(result, vec!["vision-provider", "text-provider"]);
    }

    // ── NEW: Audio capability detection tests ────────────────────────

    #[test]
    fn test_detect_capabilities_with_audio() {
        let req = ChatCompletionRequest {
            model: "test".into(),
            messages: vec![Message {
                role: "user".into(),
                content: Some(Content::Parts(vec![ContentPart {
                    part_type: "input_audio".into(),
                    text: None,
                    image_url: None,
                }])),
                name: None, tool_calls: None, tool_call_id: None,
            }],
            ..Default::default()
        };
        let caps = RouteEngine::detect_capabilities(&req);
        assert_eq!(caps, vec!["audio"]);
    }

    #[test]
    fn test_detect_capabilities_mixed_vision_audio() {
        let req = ChatCompletionRequest {
            model: "test".into(),
            messages: vec![Message {
                role: "user".into(),
                content: Some(Content::Parts(vec![
                    ContentPart {
                        part_type: "image_url".into(), text: None,
                        image_url: Some(ImageUrl { url: "data:...".into(), detail: None }),
                    },
                    ContentPart {
                        part_type: "input_audio".into(), text: None, image_url: None,
                    },
                ])),
                name: None, tool_calls: None, tool_call_id: None,
            }],
            ..Default::default()
        };
        let caps = RouteEngine::detect_capabilities(&req);
        assert!(caps.contains(&"vision"));
        assert!(caps.contains(&"audio"));
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn test_detect_capabilities_multiple_images_dedup() {
        let req = ChatCompletionRequest {
            model: "test".into(),
            messages: vec![Message {
                role: "user".into(),
                content: Some(Content::Parts(vec![
                    ContentPart {
                        part_type: "image_url".into(), text: None,
                        image_url: Some(ImageUrl { url: "data:img1".into(), detail: None }),
                    },
                    ContentPart {
                        part_type: "image_url".into(), text: None,
                        image_url: Some(ImageUrl { url: "data:img2".into(), detail: None }),
                    },
                ])),
                name: None, tool_calls: None, tool_call_id: None,
            }],
            ..Default::default()
        };
        let caps = RouteEngine::detect_capabilities(&req);
        assert_eq!(caps, vec!["vision"]);
    }

    #[test]
    fn test_detect_capabilities_messages_span_multiple() {
        let req = ChatCompletionRequest {
            model: "test".into(),
            messages: vec![
                Message {
                    role: "system".into(),
                    content: Some(Content::Text("You are helpful.".into())),
                    name: None, tool_calls: None, tool_call_id: None,
                },
                Message {
                    role: "user".into(),
                    content: Some(Content::Parts(vec![ContentPart {
                        part_type: "image_url".into(), text: None,
                        image_url: Some(ImageUrl { url: "data:pic".into(), detail: None }),
                    }])),
                    name: None, tool_calls: None, tool_call_id: None,
                },
            ],
            ..Default::default()
        };
        let caps = RouteEngine::detect_capabilities(&req);
        assert_eq!(caps, vec!["vision"]);
    }

    // ── NEW: Audio reorder capability tests ──────────────────────────

    #[test]
    fn test_reorder_by_capability_audio_provider_prioritized() {
        let mut registry = ProviderRegistry::new();
        registry.capabilities.insert("vision-provider".into(), vec!["vision".into()]);
        registry.capabilities.insert("audio-provider".into(), vec!["audio".into()]);
        registry.capabilities.insert("text-provider".into(), vec![]);

        let providers = vec!["text-provider".into(), "vision-provider".into(), "audio-provider".into()];
        let result = RouteEngine::reorder_by_capability(providers.clone(), &["audio"], &registry);
        assert_eq!(result[0], "audio-provider", "audio provider should be first");
        // text should be last (no capabilities)
        assert_eq!(result[2], "vision-provider");
    }

    #[test]
    fn test_reorder_by_capability_multiple_matches_preserves_order() {
        let mut registry = ProviderRegistry::new();
        registry.capabilities.insert("cap-a".into(), vec!["vision".into()]);
        registry.capabilities.insert("cap-b".into(), vec!["vision".into(), "audio".into()]);
        registry.capabilities.insert("no-cap".into(), vec![]);

        let providers = vec!["no-cap".into(), "cap-a".into(), "cap-b".into()];
        let result = RouteEngine::reorder_by_capability(providers, &["vision"], &registry);
        // Both vision-capable providers should be before no-cap
        let pos_a = result.iter().position(|p| p == "cap-a").unwrap();
        let pos_b = result.iter().position(|p| p == "cap-b").unwrap();
        let pos_none = result.iter().position(|p| p == "no-cap").unwrap();
        assert!(pos_a < pos_none, "cap-a should be before no-cap");
        assert!(pos_b < pos_none, "cap-b should be before no-cap");
        // Among matches, original order preserved
        assert!(pos_a < pos_b, "cap-a should be before cap-b (original order)");
    }

    #[test]
    fn test_reorder_by_capability_no_matches_unchanged() {
        let mut registry = ProviderRegistry::new();
        registry.capabilities.insert("only-vision".into(), vec!["vision".into()]);

        let providers = vec!["only-vision".into()];
        let result = RouteEngine::reorder_by_capability(providers.clone(), &["audio"], &registry);
        assert_eq!(result, providers, "order unchanged when no provider has the capability");
    }

    #[test]
    fn test_reorder_by_capability_provider_with_multiple_caps() {
        let mut registry = ProviderRegistry::new();
        registry.capabilities.insert("multi-cap".into(), vec!["vision".into(), "audio".into()]);
        registry.capabilities.insert("vision-only".into(), vec!["vision".into()]);

        let providers = vec!["vision-only".into(), "multi-cap".into()];
        let result = RouteEngine::reorder_by_capability(providers, &["vision", "audio"], &registry);
        // Both match; original order preserved among matches
        assert_eq!(result[0], "vision-only");
        assert_eq!(result[1], "multi-cap");
    }
}
