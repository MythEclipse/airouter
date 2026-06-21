// ─── Integration tests ──────────────────────────────────────────────

#[test]
fn test_health_check_endpoint() {
    // Placeholder: verify the app structure
}

#[test]
fn test_chat_completion_request_routing() {
    use airouter::config::settings::{Settings, default_providers, default_routes};
    use airouter::provider::ProviderRegistry;

    let providers = default_providers();
    let registry = ProviderRegistry::from_config(&providers);

    // Verify 9 providers registered (2 free + 1 free-tier + 6 api-key)
    assert_eq!(registry.all().count(), 20);

    // Verify routes
    let routes = default_routes();
    assert!(routes.iter().any(|r| r.model == "deepseek-v4-flash-free"));
    assert!(routes.iter().any(|r| r.model == "mimo-auto"));
    assert!(routes.iter().any(|r| r.model == "gpt-4o"));
    assert!(routes.iter().any(|r| r.model == "deepseek-chat"));
}

#[test]
fn test_auth_rejection() {
    use airouter::auth::{extract_bearer_token, validate_key};
    use axum::http::HeaderMap;

    let headers = HeaderMap::new();
    assert!(extract_bearer_token(&headers).is_none());

    let keys = vec!["sk-test-abc123".into()];
    assert!(!validate_key("invalid", &keys));
}

#[test]
fn test_auth_acceptance() {
    use airouter::auth::validate_key;

    let keys = vec!["sk-test-abc123".into(), "sk-prod-xyz789".into()];
    assert!(validate_key("sk-test-abc123", &keys));
    assert!(validate_key("sk-prod-xyz789", &keys));
}

#[test]
fn test_rate_limit_config_default() {
    use airouter::config::settings::RateLimitConfig;

    let rl = RateLimitConfig::default();
    assert!(rl.enabled);
    assert_eq!(rl.requests_per_minute, 60);
    assert_eq!(rl.burst_size, 20);
}

#[test]
fn test_provider_registry_from_defaults() {
    use airouter::provider::ProviderRegistry;
    use airouter::config::settings::default_providers;

    let providers = default_providers();
    let registry = ProviderRegistry::from_config(&providers);

    let opencode = registry.get("opencode");
    assert!(opencode.is_some());
    assert_eq!(opencode.unwrap().provider_type(), "opencode_free");

    let mimo = registry.get("mimo");
    assert!(mimo.is_some());
    assert_eq!(mimo.unwrap().provider_type(), "mimo_free");

    let openai = registry.get("openai");
    assert!(openai.is_some());
    assert_eq!(openai.unwrap().provider_type(), "openai");

    let deepseek = registry.get("deepseek");
    assert!(deepseek.is_some());
    assert_eq!(deepseek.unwrap().provider_type(), "deepseek");
}

#[test]
fn test_provider_registry_unknown_type_falls_back() {
    use airouter::config::settings::ProviderConfig;
    use airouter::provider::ProviderRegistry;
    use std::collections::HashMap;

    let providers = vec![ProviderConfig {
        name: "custom".into(),
        provider_type: "nonexistent_type".into(),
        api_key: String::new(),
        base_url: "http://localhost".into(),
        models: vec!["model-1".into()],
        extra_headers: HashMap::new(),
        capabilities: Vec::new(),
    }];

    let registry = ProviderRegistry::from_config(&providers);
    let p = registry.get("custom");
    assert!(p.is_some());
    assert_eq!(p.unwrap().provider_type(), "openai_compat");
}

#[test]
fn test_all_routes_have_provider() {
    use airouter::config::settings::{default_providers, default_routes};
    use airouter::provider::ProviderRegistry;

    let registry = ProviderRegistry::from_config(&default_providers());

    for route in default_routes() {
        if let Some(provider_name) = &route.provider {
            let found = registry.get(provider_name);
            assert!(found.is_some(), "Route '{}' references missing provider '{}'", route.model, provider_name);
        }
        if let Some(providers) = &route.providers {
            for p in providers {
                let found = registry.get(p);
                assert!(found.is_some(), "Route '{}' references missing provider '{}'", route.model, p);
            }
        }
    }
}

#[test]
fn test_no_duplicate_routes() {
    use airouter::config::settings::default_routes;

    let routes = default_routes();
    let mut seen = std::collections::HashSet::new();
    for route in &routes {
        if !seen.insert(&route.model) {
            panic!("Duplicate route for model '{}'", route.model);
        }
    }
}

#[test]
fn test_model_names_unique_per_provider() {
    use airouter::config::settings::default_providers;

    for p in default_providers() {
        let mut seen = std::collections::HashSet::new();
        for m in &p.models {
            if !seen.insert(m) {
                panic!("Duplicate model '{}' in provider '{}'", m, p.name);
            }
        }
    }
}

// ─── OpenAI ↔ Anthropic format translation ───────────────────────────

#[test]
fn test_openai_to_anthropic_transform() {
    use airouter::types::openai::{ChatCompletionRequest, Message, Content};
    use airouter::transform::openai_to_anthropic::convert_chat_request;

    let req = ChatCompletionRequest {
        model: "claude-sonnet-4-20250514".into(),
        messages: vec![
            Message {
                role: "user".into(),
                content: Some(Content::Text("What is Rust?".into())),
                name: None, tool_calls: None, tool_call_id: None,
            },
        ],
        stream: None, temperature: Some(0.5), max_tokens: Some(1024), top_p: None,
        frequency_penalty: None, presence_penalty: None, stop: None,
        tools: None, tool_choice: None, user: None, metadata: None,
    };

    let anthro = convert_chat_request(&req).unwrap();
    assert_eq!(anthro.model, "claude-sonnet-4-20250514");
    assert_eq!(anthro.temperature, Some(0.5));
    assert_eq!(anthro.max_tokens, Some(1024));
}

#[test]
fn test_anthropic_to_openai_transform() {
    use airouter::types::anthropic::*;
    use airouter::transform::anthropic_to_openai::convert_messages_request;

    let req = MessagesRequest {
        model: "gpt-4o".into(),
        messages: vec![
            AnthropicMessage {
                role: "user".into(),
                content: AnthropicContent::Text("Explain monads".into()),
            },
        ],
        stream: None, max_tokens: Some(2048), system: None,
        temperature: None, top_p: None, top_k: None,
        stop_sequences: None, tools: None, metadata: None,
    };

    let openai = convert_messages_request(&req);
    assert_eq!(openai.model, "gpt-4o");
    assert_eq!(openai.max_tokens, Some(2048));
    assert!(!openai.messages.is_empty());
    assert_eq!(openai.messages[0].role, "user");
}

// ─── Default provider/routes tests ──────────────────────────────────

#[test]
fn test_default_providers_count() {
    use airouter::config::settings::default_providers;

    let bp = default_providers();
    assert_eq!(bp.len(), 20, "Expected 20 default providers (2 free + 1 free-tier + 6 api-key + 11 new)");
    assert!(bp.iter().any(|p| p.name == "opencode"));
    assert!(bp.iter().any(|p| p.name == "mimo"));
    assert!(bp.iter().any(|p| p.name == "openai"));
    assert!(bp.iter().any(|p| p.name == "anthropic"));
    assert!(bp.iter().any(|p| p.name == "deepseek"));
    assert!(bp.iter().any(|p| p.name == "gemini"));
    assert!(bp.iter().any(|p| p.name == "groq"));
    assert!(bp.iter().any(|p| p.name == "ollama"));
}

#[test]
fn test_models_list_contains_all_models() {
    use airouter::provider::ProviderRegistry;
    use airouter::config::settings::default_providers;

    let registry = ProviderRegistry::from_config(&default_providers());
    for provider in registry.all() {
        for model in provider.models() {
            assert!(!model.is_empty(), "Empty model name in provider '{}'", provider.name());
        }
    }
}

#[test]
fn test_provider_count_and_types() {
    use airouter::provider::ProviderRegistry;
    use airouter::config::settings::default_providers;

    let registry = ProviderRegistry::from_config(&default_providers());
    let count = registry.all().count();
    assert_eq!(count, 20, "Expected exactly 20 providers");

    let mut types: Vec<&str> = registry.all().map(|p| p.provider_type()).collect();
    types.sort();
    // 20 provider types sorted
    assert_eq!(types.len(), 20);
    assert!(types.contains(&"anthropic"));
    assert!(types.contains(&"deepseek"));
    assert!(types.contains(&"gemini"));
    assert!(types.contains(&"groq"));
    assert!(types.contains(&"mimo_free"));
    assert!(types.contains(&"ollama"));
    assert!(types.contains(&"opencode_free"));
    assert!(types.contains(&"openai"));
    assert!(types.contains(&"openrouter"));
}
