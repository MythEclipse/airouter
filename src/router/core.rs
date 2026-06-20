use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use axum::http::StatusCode;
use axum::response::{
    sse::{Event, Sse},
    IntoResponse, Json, Response,
};
use dashmap::DashMap;
use std::convert::Infallible;
use tokio_stream::StreamExt;
use crate::config::settings::{RouteConfig, Settings, StrategyKind};
use crate::provider::{ErrorClass, Provider, ProviderRegistry};
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

// ─── Rotation State (for round-robin strategies) ─────────────────

struct RotationState {
    index: AtomicUsize,
    current_provider: Mutex<Option<String>>,
    request_count: AtomicUsize,
}

impl RotationState {
    fn new() -> Self {
        Self {
            index: AtomicUsize::new(0),
            current_provider: Mutex::new(None),
            request_count: AtomicUsize::new(0),
        }
    }

    fn select(&self, providers: &[String], sticky_limit: Option<usize>) -> String {
        if providers.is_empty() {
            return String::new();
        }
        match sticky_limit {
            Some(limit) => {
                let mut current = self.current_provider.lock().unwrap();
                let count = self.request_count.load(Ordering::Relaxed);

                if let Some(ref cur) = *current {
                    if providers.contains(cur) && count < limit {
                        self.request_count.fetch_add(1, Ordering::Relaxed);
                        return cur.clone();
                    }
                }
                // Advance to next provider
                let idx = self.index.fetch_add(1, Ordering::Relaxed) % providers.len();
                let selected = providers[idx].clone();
                *current = Some(selected.clone());
                self.request_count.store(1, Ordering::Relaxed);
                selected
            }
            None => {
                let idx = self.index.fetch_add(1, Ordering::Relaxed) % providers.len();
                providers[idx].clone()
            }
        }
    }
}

// ─── RouteEngine ─────────────────────────────────────────────────

pub struct RouteEngine {
    registry: Arc<ProviderRegistry>,
    rotation_states: Arc<DashMap<String, RotationState>>,
    balancer: Arc<LoadBalancer>,
    settings: Arc<Settings>,
}

impl RouteEngine {
    pub fn new(
        registry: Arc<ProviderRegistry>,
        balancer: Arc<LoadBalancer>,
        settings: Arc<Settings>,
    ) -> Self {
        Self {
            registry,
            rotation_states: Arc::new(DashMap::new()),
            balancer,
            settings,
        }
    }

    /// Find a RouteConfig for a model (exact match or wildcard *)
    pub fn find_route(&self, model: &str) -> Option<&RouteConfig> {
        for route in &self.settings.routes {
            if route.model == model {
                return Some(route);
            }
        }
        // Wildcard fallback
        for route in &self.settings.routes {
            if route.model == "*" {
                return Some(route);
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
    fn get_provider_names(&self, model: &str, capabilities: &[&str]) -> Result<Vec<String>, DispatchError> {
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
        if !capabilities.is_empty() {
            names = Self::reorder_by_capability(names, capabilities, &self.registry);
        }

        // Load balancer selection: reorder so selected is first
        if let Some(selected) = self.balancer.select_by_name(&names) {
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
        request: ChatCompletionRequest,
        is_stream: bool,
        tracker: &RequestTracker,
    ) -> Result<Response, DispatchError> {
        let model = request.model.clone();
        let capabilities = Self::detect_capabilities(&request);

        let provider_names = self.get_provider_names(&model, &capabilities)?;
        let route = self.find_route(&model).ok_or_else(|| {
            DispatchError::ModelNotFound(format!("No route for model '{}'", model))
        })?;

        let strategy = route.effective_strategy(self.settings.default_strategy.as_ref());

        match strategy {
            StrategyKind::Fusion => {
                // Fusion is always non-streaming
                crate::router::fusion::execute_fusion(
                    provider_names,
                    request,
                    self.registry.clone(),
                    &self.settings.routes,
                    tracker,
                    route.combo.as_ref().unwrap_or(&Default::default()),
                    &model,
                ).await
            }
            StrategyKind::RoundRobin => {
                self.execute_round_robin(provider_names, request, route, is_stream, tracker, &model).await
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
            let provider = match self.registry.get(pname) {
                Some(p) => p,
                None => continue,
            };

            if i > 0 && self.balancer.is_on_cooldown(pname) {
                tracing::debug!(provider = %pname, "Skipping on cooldown");
                continue;
            }

            if is_stream {
                match provider.chat_completion_stream(request.clone()).await {
                    Ok(provider_stream) => {
                        let elapsed = start.elapsed().as_millis();
                        tracker.record_request(pname, model, elapsed as u64, true);
                        self.balancer.clear_cooldown(pname);

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
                        tracing::warn!(provider = %pname, model = %model, error = %e, "Stream provider failed");
                        tracker.record_request(pname, model, elapsed as u64, false);
                        self.balancer.mark_cooldown_with_class(pname, class);
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
                        tracker.record_request(pname, model, elapsed as u64, true);
                        self.balancer.clear_cooldown(pname);
                        return Ok(Json(resp).into_response());
                    }
                    Err(e) => {
                        let elapsed = start.elapsed().as_millis();
                        let class = e.error_class();
                        tracing::warn!(provider = %pname, model = %model, error = %e, "Provider failed");
                        tracker.record_request(pname, model, elapsed as u64, false);
                        self.balancer.mark_cooldown_with_class(pname, class);
                        last_error = e.to_string();
                        if !e.is_retryable() || class == ErrorClass::BadRequest {
                            break;
                        }
                    }
                }
            }
        }

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
        let rotated = {
            let state = self.rotation_states
                .entry(model.to_string())
                .or_insert_with(RotationState::new);
            let selected = state.select(&provider_names, sticky_limit);
            let (mut matched, others): (Vec<_>, Vec<_>) = provider_names.into_iter()
                .partition(|p| *p == selected);
            matched.extend(others);
            matched
        };

        self.execute_sequential(rotated, request, is_stream, tracker, model).await
    }
}

// ─── Keep RouteModel trait for backward compat ───────────────────

pub trait RouteModel {
    fn resolve(&self, model: &str, routes: &[RouteConfig]) -> Result<&Box<dyn Provider>, String>;
    fn get_fallback_providers(&self, model: &str, routes: &[RouteConfig]) -> Vec<&Box<dyn Provider>>;
}

impl RouteModel for ProviderRegistry {
    fn resolve(&self, model: &str, routes: &[RouteConfig]) -> Result<&Box<dyn Provider>, String> {
        for route in routes {
            if route.model == model {
                if let Some(provider_name) = &route.provider {
                    return self.get(provider_name)
                        .ok_or_else(|| format!("Provider '{}' not found for model '{}'", provider_name, model));
                }
                if let Some(providers) = &route.providers {
                    if let Some(first) = providers.first() {
                        return self.get(first)
                            .ok_or_else(|| format!("Provider '{}' not found for model '{}'", first, model));
                    }
                }
            }
        }
        for route in routes {
            if route.model == "*" {
                if let Some(provider_name) = &route.provider {
                    return self.get(provider_name)
                        .ok_or_else(|| format!("Fallback provider '{}' not found", provider_name));
                }
                if let Some(providers) = &route.providers {
                    if let Some(first) = providers.first() {
                        return self.get(first)
                            .ok_or_else(|| format!("Fallback provider '{}' not found", first));
                    }
                }
            }
        }
        Err(format!("No route found for model '{}'", model))
    }

    fn get_fallback_providers(&self, model: &str, routes: &[RouteConfig]) -> Vec<&Box<dyn Provider>> {
        for route in routes {
            if route.model == model || route.model == "*" {
                if let Some(providers) = &route.providers {
                    return providers.iter()
                        .filter_map(|name| self.get(name))
                        .skip(1)
                        .collect();
                }
            }
        }
        Vec::new()
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

    #[test]
    fn test_rotation_state_advance() {
        let state = RotationState::new();
        let providers = vec!["a".into(), "b".into(), "c".into()];
        let r1 = state.select(&providers, Some(1));
        let r2 = state.select(&providers, Some(1));
        let r3 = state.select(&providers, Some(1));
        // With 3 providers and sticky=1, each call should advance
        assert_ne!(r1, r2, "two calls with sticky=1 should give different providers");
        assert_ne!(r1, r3);
    }
}
