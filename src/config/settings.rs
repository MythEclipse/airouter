use serde::Deserialize;
use std::fs;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { host: "0.0.0.0".into(), port: 3000 }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouteConfig {
    pub model: String,
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub providers: Option<Vec<String>>,
}

fn default_enabled() -> bool { true }
fn default_rpm() -> u64 { 60 }
fn default_burst() -> u32 { 20 }

#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u64,
    #[serde(default = "default_burst")]
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self { enabled: true, requests_per_minute: 60, burst_size: 20 }
    }
}

/// Free built-in providers — always available, no config needed.
pub fn builtin_providers() -> Vec<ProviderConfig> {
    vec![
        ProviderConfig {
            name: "opencode".into(),
            provider_type: "opencode_free".into(),
            api_key: String::new(),
            base_url: String::new(),
            models: vec![
                "kimi-k2.6".into(), "kimi-k2.5".into(),
                "glm-5.1".into(), "glm-5".into(),
                "qwen3.5-plus".into(), "qwen3.6-plus".into(),
                "mimo-v2-pro".into(), "mimo-v2-omni".into(),
                "minimax-m2.7".into(), "minimax-m2.5".into(),
            ],
            extra_headers: HashMap::new(),
        },
        ProviderConfig {
            name: "mimo".into(),
            provider_type: "mimo_free".into(),
            api_key: String::new(),
            base_url: String::new(),
            models: vec![
                "mimo-v2.5-pro".into(), "mimo-v2.5".into(),
                "mimo-v2-omni".into(), "mimo-v2-flash".into(),
            ],
            extra_headers: HashMap::new(),
        },
    ]
}

/// Free built-in routes — always available.
pub fn builtin_routes() -> Vec<RouteConfig> {
    vec![
        // OpenCode Free models
        RouteConfig { model: "kimi-k2.6".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "kimi-k2.5".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "glm-5.1".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "glm-5".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "qwen3.5-plus".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "qwen3.6-plus".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "mimo-v2-pro".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "mimo-v2-omni".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "minimax-m2.7".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        RouteConfig { model: "minimax-m2.5".into(), strategy: "single".into(), provider: Some("opencode".into()), providers: None },
        // MiMo Free models (unique to mimo, not in opencode)
        RouteConfig { model: "mimo-v2.5-pro".into(), strategy: "single".into(), provider: Some("mimo".into()), providers: None },
        RouteConfig { model: "mimo-v2.5".into(), strategy: "single".into(), provider: Some("mimo".into()), providers: None },
        RouteConfig { model: "mimo-v2-flash".into(), strategy: "single".into(), provider: Some("mimo".into()), providers: None },
    ]
}

impl Settings {
    pub fn from_file(path: &str) -> Result<Self, anyhow::Error> {
        let contents = fs::read_to_string(path)?;
        let mut settings: Settings = serde_yaml::from_str(&contents)?;
        for provider in &mut settings.providers {
            provider.api_key = resolve_env(&provider.api_key);
        }
        // Merge built-in free providers
        for bp in builtin_providers() {
            if !settings.providers.iter().any(|p| p.name == bp.name) {
                settings.providers.insert(0, bp);
            }
        }
        // Merge built-in free routes
        for br in builtin_routes() {
            if !settings.routes.iter().any(|r| r.model == br.model) {
                settings.routes.push(br);
            }
        }
        Ok(settings)
    }

    /// Default settings with no config file — free providers only.
    pub fn default_builtins() -> Self {
        Self {
            server: ServerConfig::default(),
            keys: vec!["sk-test-abc123".into()],
            providers: builtin_providers(),
            routes: builtin_routes(),
            rate_limit: RateLimitConfig::default(),
        }
    }

    /// Try loading from file, fall back to builtins if file not found.
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        if fs::metadata(path).is_ok() {
            Self::from_file(path)
        } else {
            tracing::warn!(path = %path, "Config file not found, using built-in free providers");
            Ok(Self::default_builtins())
        }
    }
}

fn resolve_env(val: &str) -> String {
    if val.starts_with("${") && val.ends_with("}") {
        let var = &val[2..val.len()-1];
        std::env::var(var).unwrap_or_default()
    } else {
        val.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env() {
        std::env::set_var("TEST_VAR_123", "hello");
        assert_eq!(resolve_env("${TEST_VAR_123}"), "hello");
        assert_eq!(resolve_env("literal-key"), "literal-key");
        assert_eq!(resolve_env(""), "");
    }

    #[test]
    fn test_default_settings() {
        let s = Settings::default_builtins();
        assert_eq!(s.server.port, 3000);
        assert!(s.providers.iter().any(|p| p.name == "opencode"));
        assert!(s.providers.iter().any(|p| p.name == "mimo"));
        assert!(s.routes.iter().any(|r| r.model == "kimi-k2.6"));
        assert!(s.routes.iter().any(|r| r.model == "mimo-v2.5-pro"));
    }

    #[test]
    fn test_builtin_providers_count() {
        let bp = builtin_providers();
        assert_eq!(bp.len(), 2);
        assert_eq!(bp[0].provider_type, "opencode_free");
        assert_eq!(bp[1].provider_type, "mimo_free");
    }

    #[test]
    fn test_builtin_routes_count() {
        let br = builtin_routes();
        assert_eq!(br.len(), 13);
    }

    #[test]
    fn test_rate_limit_config_default() {
        let rl = RateLimitConfig::default();
        assert!(rl.enabled);
        assert_eq!(rl.requests_per_minute, 60);
        assert_eq!(rl.burst_size, 20);
    }

    #[test]
    fn test_server_config_default() {
        let sc = ServerConfig::default();
        assert_eq!(sc.host, "0.0.0.0");
        assert_eq!(sc.port, 3000);
    }
}
