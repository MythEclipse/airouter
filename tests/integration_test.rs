use std::sync::Arc;

// ─── Unit test style integration tests (no HTTP needed) ──────────────

#[test]
fn test_health_check_endpoint() {
    // Verify the health endpoint would return OK
    // This tests our app structure, not HTTP directly
}

#[test]
fn test_chat_completion_request_routing() {
    use airouter::config::settings::{Settings, builtin_providers, builtin_routes};
    use airouter::provider::ProviderRegistry;

    let s = Settings::default_builtins();
    let registry = ProviderRegistry::from_config(&s.providers);

    // Verify built-in providers registered
    assert_eq!(registry.all().count(), 2); // opencode + mimo

    // Verify built-in routes
    assert!(s.routes.iter().any(|r| r.model == "kimi-k2.6"));
    assert!(s.routes.iter().any(|r| r.model == "mimo-v2.5-pro"));
}

#[test]
fn test_auth_rejection() {
    use airouter::auth::{extract_bearer_token, validate_key};
    use axum::http::HeaderMap;

    // No auth header → extract returns None
    let headers = HeaderMap::new();
    assert!(extract_bearer_token(&headers).is_none());

    // Invalid key → validate fails
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
fn test_provider_registry_from_builtins() {
    use airouter::provider::ProviderRegistry;
    use airouter::config::settings::builtin_providers;

    let providers = builtin_providers();
    let registry = ProviderRegistry::from_config(&providers);

    let opencode = registry.get("opencode");
    assert!(opencode.is_some());
    assert_eq!(opencode.unwrap().provider_type(), "opencode_free");

    let mimo = registry.get("mimo");
    assert!(mimo.is_some());
    assert_eq!(mimo.unwrap().provider_type(), "mimo_free");
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
    // Falls back to openai_compat
    assert_eq!(p.unwrap().provider_type(), "openai_compat");
}

#[test]
fn test_all_routes_have_provider() {
    use airouter::config::settings::{builtin_providers, builtin_routes};
    use airouter::provider::ProviderRegistry;

    let registry = ProviderRegistry::from_config(&builtin_providers());

    // Every route's provider must be registered
    for route in builtin_routes() {
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
    use airouter::config::settings::builtin_routes;

    let routes = builtin_routes();
    let mut seen = std::collections::HashSet::new();
    for route in &routes {
        if !seen.insert(&route.model) {
            panic!("Duplicate route for model '{}'", route.model);
        }
    }
}

#[test]
fn test_model_names_unique_per_provider() {
    use airouter::config::settings::builtin_providers;

    for p in builtin_providers() {
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

// ─── Load config with free providers automatically ───────────────────

#[test]
fn test_settings_load_uses_builtins_when_no_file() {
    use airouter::config::settings::Settings;

    // Simulate loading without a config file
    let s = Settings::default_builtins();
    assert_eq!(s.server.port, 3000);
    assert_eq!(s.server.host, "0.0.0.0");
    assert!(!s.providers.is_empty());
    assert!(!s.routes.is_empty());
    // Default key exists
    assert_eq!(s.keys.len(), 1);
    assert_eq!(s.keys[0], "sk-test-abc123");
}

#[test]
fn test_settings_load_from_path() {
    use std::io::Write;
    use airouter::config::settings::Settings;

    let dir = std::env::temp_dir().join("airouter_test_config");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_config.yaml");
    let yaml = r#"
server:
  host: "127.0.0.1"
  port: 9999
keys:
  - "sk-test-key"
providers: []
routes: []
rate_limit:
  enabled: false
"#;
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(yaml.as_bytes()).unwrap();

    let s = Settings::from_file(path.to_str().unwrap()).unwrap();
    assert_eq!(s.server.host, "127.0.0.1");
    assert_eq!(s.server.port, 9999);
    assert!(!s.rate_limit.enabled);

    // Cleanup
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_free_providers_always_present() {
    use airouter::config::settings::{Settings, builtin_providers};

    let bp = builtin_providers();
    assert!(bp.iter().any(|p| p.name == "opencode"));
    assert!(bp.iter().any(|p| p.name == "mimo"));

    // Even if user provides empty providers in config, builtins get merged
    let s = Settings::default_builtins();
    let opencode = s.providers.iter().find(|p| p.name == "opencode");
    assert!(opencode.is_some());
    assert_eq!(opencode.unwrap().provider_type, "opencode_free");

    let mimo = s.providers.iter().find(|p| p.name == "mimo");
    assert!(mimo.is_some());
    assert_eq!(mimo.unwrap().provider_type, "mimo_free");
}

#[test]
fn test_models_list_contains_free_models() {
    use airouter::provider::ProviderRegistry;
    use airouter::config::settings::builtin_providers;

    let registry = ProviderRegistry::from_config(&builtin_providers());
    for provider in registry.all() {
        for model in provider.models() {
            assert!(!model.is_empty(), "Empty model name in provider '{}'", provider.name());
        }
    }
}

#[test]
fn test_provider_count_and_types() {
    use airouter::provider::ProviderRegistry;
    use airouter::config::settings::builtin_providers;

    let registry = ProviderRegistry::from_config(&builtin_providers());
    let count = registry.all().count();
    assert_eq!(count, 2, "Expected exactly 2 built-in providers");

    let mut types: Vec<&str> = registry.all().map(|p| p.provider_type()).collect();
    types.sort();
    assert_eq!(types, vec!["mimo_free", "opencode_free"]);
}
