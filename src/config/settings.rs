use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StrategyKind {
    Single,
    Fallback,
    #[serde(rename = "round-robin")]
    RoundRobin,
    Fusion,
}

impl Default for StrategyKind {
    fn default() -> Self {
        Self::Fallback
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ComboConfig {
    #[serde(default)]
    pub judge_model: Option<String>,
    #[serde(default = "default_min_panel")]
    pub min_panel: usize,
    #[serde(default = "default_straggler_grace_ms")]
    pub straggler_grace_ms: u64,
    #[serde(default = "default_panel_hard_timeout_ms")]
    pub panel_hard_timeout_ms: u64,
    #[serde(default)]
    pub sticky_limit: Option<usize>,
}

impl Default for ComboConfig {
    fn default() -> Self {
        Self {
            judge_model: None,
            min_panel: 1,
            straggler_grace_ms: 2000,
            panel_hard_timeout_ms: 30000,
            sticky_limit: None,
        }
    }
}

fn default_min_panel() -> usize { 1 }
fn default_straggler_grace_ms() -> u64 { 2000 }
fn default_panel_hard_timeout_ms() -> u64 { 30000 }

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerConfig,
    #[serde(default)]
    pub default_strategy: Option<StrategyKind>,
    #[serde(default)]
    pub keys: Vec<String>,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub routes: Vec<RouteConfig>,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            default_strategy: None,
            keys: vec![],
            providers: vec![],
            routes: vec![],
            rate_limit: RateLimitConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub default_max_tokens: Option<u32>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { host: "0.0.0.0".into(), port: 3000, default_max_tokens: None }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouteConfig {
    pub model: String,
    #[serde(default)]
    pub strategy: StrategyKind,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub providers: Option<Vec<String>>,
    #[serde(default)]
    pub combo: Option<ComboConfig>,
}

impl RouteConfig {
    pub fn effective_strategy(&self, global_default: Option<&StrategyKind>) -> StrategyKind {
        match &self.strategy {
            StrategyKind::Single | StrategyKind::RoundRobin | StrategyKind::Fusion => self.strategy.clone(),
            StrategyKind::Fallback => {
                global_default.cloned().unwrap_or(StrategyKind::Fallback)
            }
        }
    }
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

/// Default API key used when seeding the database
pub const DEFAULT_KEY: &str = "sk-test-abc123";

/// Default seed providers — one entry per known provider type.
/// Free providers (opencode_free, mimo_free) have empty api_key + base_url (hardcoded at runtime).
/// Paid providers use empty api_key (filled via dashboard) with known default base_url.
pub fn default_providers() -> Vec<ProviderConfig> {
    vec![
        // ── FREE (no auth) ──────────────────────────────────────
        ProviderConfig {
            name: "opencode".into(),
            provider_type: "opencode_free".into(),
            api_key: String::new(),
            base_url: String::new(),  // hardcoded in provider impl
            models: vec![
                "deepseek-v4-flash-free".into(),
                "mimo-v2.5-free".into(),
                "nemotron-3-ultra-free".into(),
                "north-mini-code-free".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: vec!["vision".into()],
        },
        ProviderConfig {
            name: "mimo".into(),
            provider_type: "mimo_free".into(),
            api_key: String::new(),
            base_url: String::new(),  // hardcoded in provider impl
            models: vec![
                "mimo-auto".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        // ── FREE TIER ────────────────────────────────────────────
        ProviderConfig {
            name: "gemini".into(),
            provider_type: "gemini".into(),
            api_key: String::new(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
            models: vec![
                "gemini-2.5-pro-exp-03-25".into(),
                "gemini-2.0-flash".into(),
                "gemini-1.5-flash".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: vec!["vision".into()],
        },
        ProviderConfig {
            name: "groq".into(),
            provider_type: "groq".into(),
            api_key: String::new(),
            base_url: "https://api.groq.com/openai/v1".into(),
            models: vec![
                "llama-3.3-70b-versatile".into(),
                "llama-3.1-8b-instant".into(),
                "mixtral-8x7b-32768".into(),
                "deepseek-r1-distill-llama-70b".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        // ── API KEY ──────────────────────────────────────────────
        ProviderConfig {
            name: "openai".into(),
            provider_type: "openai".into(),
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".into(),
            models: vec![
                "gpt-4o".into(), "gpt-4o-mini".into(),
                "o3".into(), "o4-mini".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: vec!["vision".into(), "audio".into()],
        },
        ProviderConfig {
            name: "anthropic".into(),
            provider_type: "anthropic".into(),
            api_key: String::new(),
            base_url: "https://api.anthropic.com/v1".into(),
            models: vec![
                "claude-sonnet-4-20250514".into(),
                "claude-3-5-sonnet-20241022".into(),
                "claude-3-haiku-20240307".into(),
                "claude-opus-4-20250514".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: vec!["vision".into()],
        },
        ProviderConfig {
            name: "deepseek".into(),
            provider_type: "deepseek".into(),
            api_key: String::new(),
            base_url: "https://api.deepseek.com/v1".into(),
            models: vec!["deepseek-chat".into(), "deepseek-reasoner".into()],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "openrouter".into(),
            provider_type: "openrouter".into(),
            api_key: String::new(),
            base_url: "https://openrouter.ai/api/v1".into(),
            models: vec![
                "openai/gpt-4o".into(),
                "anthropic/claude-sonnet-4".into(),
                "google/gemini-2.0-flash".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "ollama".into(),
            provider_type: "ollama".into(),
            api_key: String::new(),
            base_url: "http://localhost:11434/v1".into(),
            models: vec!["llama3.2".into(), "mistral".into()],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        // ── MORE FREE TIER ──────────────────────────────────────────
        ProviderConfig {
            name: "mistral".into(),
            provider_type: "mistral".into(),
            api_key: String::new(),
            base_url: "https://api.mistral.ai/v1".into(),
            models: vec![
                "mistral-large-latest".into(),
                "codestral-latest".into(),
                "mistral-embed".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: vec!["vision".into()],
        },
        ProviderConfig {
            name: "together".into(),
            provider_type: "together".into(),
            api_key: String::new(),
            base_url: "https://api.together.xyz/v1".into(),
            models: vec![
                "meta-llama/Llama-3.3-70B-Instruct-Turbo".into(),
                "deepseek-ai/DeepSeek-R1".into(),
                "Qwen/Qwen3-235B-A22B".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "fireworks".into(),
            provider_type: "fireworks".into(),
            api_key: String::new(),
            base_url: "https://api.fireworks.ai/inference/v1".into(),
            models: vec![
                "accounts/fireworks/models/deepseek-v3p1".into(),
                "accounts/fireworks/models/llama-v3p3-70b-instruct".into(),
                "accounts/fireworks/models/qwen3-235b-a22b".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "xai".into(),
            provider_type: "xai".into(),
            api_key: String::new(),
            base_url: "https://api.x.ai/v1".into(),
            models: vec![
                "grok-4".into(),
                "grok-3".into(),
                "grok-4-fast-reasoning".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "cohere".into(),
            provider_type: "cohere".into(),
            api_key: String::new(),
            base_url: "https://api.cohere.ai/v1".into(),
            models: vec![
                "command-r-plus".into(),
                "command-r".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "perplexity".into(),
            provider_type: "perplexity".into(),
            api_key: String::new(),
            base_url: "https://api.perplexity.ai".into(),
            models: vec![
                "sonar-pro".into(),
                "sonar".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: vec!["web_search".into()],
        },
        ProviderConfig {
            name: "nvidia".into(),
            provider_type: "nvidia".into(),
            api_key: String::new(),
            base_url: "https://integrate.api.nvidia.com/v1".into(),
            models: vec![
                "minimaxai/minimax-m2.7".into(),
                "z-ai/glm4.7".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "siliconflow".into(),
            provider_type: "siliconflow".into(),
            api_key: String::new(),
            base_url: "https://api.siliconflow.com/v1".into(),
            models: vec![
                "deepseek-ai/DeepSeek-V4-Pro".into(),
                "deepseek-ai/DeepSeek-V4-Flash".into(),
                "Qwen/Qwen3.5-397B-A17B".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "cerebras".into(),
            provider_type: "cerebras".into(),
            api_key: String::new(),
            base_url: "https://api.cerebras.ai/v1".into(),
            models: vec![
                "llama-3.3-70b".into(),
                "llama-4-scout-17b-16e-instruct".into(),
                "qwen-3-235b-a22b-instruct-2507".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "hyperbolic".into(),
            provider_type: "hyperbolic".into(),
            api_key: String::new(),
            base_url: "https://api.hyperbolic.xyz/v1".into(),
            models: vec![
                "Qwen/QwQ-32B".into(),
                "deepseek-ai/DeepSeek-R1".into(),
                "meta-llama/Llama-3.3-70B-Instruct".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
        ProviderConfig {
            name: "cloudflare".into(),
            provider_type: "cloudflare".into(),
            api_key: String::new(),
            base_url: String::new(),  // requires account_id in URL
            models: vec![
                "@cf/meta/llama-3.3-70b-instruct-fp8-fast".into(),
            ],
            extra_headers: HashMap::new(),
            capabilities: Vec::new(),
        },
    ]
}

/// Default seed routes — one per model in default_providers, plus wildcard.
pub fn default_routes() -> Vec<RouteConfig> {
    vec![
        // OpenCode Free models
        RouteConfig { model: "deepseek-v4-flash-free".into(), strategy: StrategyKind::Single, provider: Some("opencode".into()), providers: None, combo: None },
        RouteConfig { model: "mimo-v2.5-free".into(), strategy: StrategyKind::Single, provider: Some("opencode".into()), providers: None, combo: None },
        RouteConfig { model: "nemotron-3-ultra-free".into(), strategy: StrategyKind::Single, provider: Some("opencode".into()), providers: None, combo: None },
        RouteConfig { model: "north-mini-code-free".into(), strategy: StrategyKind::Single, provider: Some("opencode".into()), providers: None, combo: None },
        // MiMo Free models
        RouteConfig { model: "mimo-auto".into(), strategy: StrategyKind::Single, provider: Some("mimo".into()), providers: None, combo: None },
        // Gemini models
        RouteConfig { model: "gemini-2.5-pro-exp-03-25".into(), strategy: StrategyKind::Single, provider: Some("gemini".into()), providers: None, combo: None },
        RouteConfig { model: "gemini-2.0-flash".into(), strategy: StrategyKind::Single, provider: Some("gemini".into()), providers: None, combo: None },
        // OpenAI models
        RouteConfig { model: "gpt-4o".into(), strategy: StrategyKind::Fallback, provider: None, providers: Some(vec!["openai".into(), "opencode".into()]), combo: None },
        RouteConfig { model: "gpt-4o-mini".into(), strategy: StrategyKind::Single, provider: Some("openai".into()), providers: None, combo: None },
        // Anthropic models
        RouteConfig { model: "claude-sonnet-4-20250514".into(), strategy: StrategyKind::Single, provider: Some("anthropic".into()), providers: None, combo: None },
        RouteConfig { model: "claude-3-5-sonnet-20241022".into(), strategy: StrategyKind::Single, provider: Some("anthropic".into()), providers: None, combo: None },
        // DeepSeek
        RouteConfig { model: "deepseek-chat".into(), strategy: StrategyKind::Single, provider: Some("deepseek".into()), providers: None, combo: None },
        // Groq
        RouteConfig { model: "llama-3.3-70b-versatile".into(), strategy: StrategyKind::Single, provider: Some("groq".into()), providers: None, combo: None },
        // Ollama
        RouteConfig { model: "llama3.2".into(), strategy: StrategyKind::Single, provider: Some("ollama".into()), providers: None, combo: None },
        // Wildcard
        RouteConfig { model: "*".into(), strategy: StrategyKind::Fallback, provider: None, providers: Some(vec!["opencode".into(), "groq".into(), "ollama".into()]), combo: None },
        // Mistral
        RouteConfig { model: "mistral-large-latest".into(), strategy: StrategyKind::Single, provider: Some("mistral".into()), providers: None, combo: None },
        RouteConfig { model: "codestral-latest".into(), strategy: StrategyKind::Single, provider: Some("mistral".into()), providers: None, combo: None },
        // Together
        RouteConfig { model: "meta-llama/Llama-3.3-70B-Instruct-Turbo".into(), strategy: StrategyKind::Single, provider: Some("together".into()), providers: None, combo: None },
        RouteConfig { model: "deepseek-ai/DeepSeek-R1".into(), strategy: StrategyKind::Fallback, provider: None, providers: Some(vec!["together".into(), "hyperbolic".into(), "siliconflow".into()]), combo: None },
        RouteConfig { model: "Qwen/Qwen3-235B-A22B".into(), strategy: StrategyKind::Single, provider: Some("together".into()), providers: None, combo: None },
        // Fireworks
        RouteConfig { model: "accounts/fireworks/models/deepseek-v3p1".into(), strategy: StrategyKind::Single, provider: Some("fireworks".into()), providers: None, combo: None },
        // xAI
        RouteConfig { model: "grok-4".into(), strategy: StrategyKind::Single, provider: Some("xai".into()), providers: None, combo: None },
        // NVIDIA
        RouteConfig { model: "minimaxai/minimax-m2.7".into(), strategy: StrategyKind::Single, provider: Some("nvidia".into()), providers: None, combo: None },
        // SiliconFlow
        RouteConfig { model: "deepseek-ai/DeepSeek-V4-Pro".into(), strategy: StrategyKind::Single, provider: Some("siliconflow".into()), providers: None, combo: None },
        RouteConfig { model: "deepseek-ai/DeepSeek-V4-Flash".into(), strategy: StrategyKind::Single, provider: Some("siliconflow".into()), providers: None, combo: None },
        // Hyperbolic
        RouteConfig { model: "Qwen/QwQ-32B".into(), strategy: StrategyKind::Single, provider: Some("hyperbolic".into()), providers: None, combo: None },
        // Cerebras
        RouteConfig { model: "llama-3.3-70b".into(), strategy: StrategyKind::Single, provider: Some("cerebras".into()), providers: None, combo: None },
        // DeepSeek R1 fallback across providers
        RouteConfig { model: "deepseek-reasoner".into(), strategy: StrategyKind::Fallback, provider: None, providers: Some(vec!["deepseek".into(), "together".into(), "siliconflow".into()]), combo: None },
    ]
}

impl Settings {
    pub fn load(path: &str) -> Result<Self, anyhow::Error> {
        // Load only the YAML for server/rate_limit/config — NOT providers/routes
        let contents = std::fs::read_to_string(path)
            .map_err(|_| anyhow::anyhow!("Config file not found: {}", path))?;
        let mut settings: Settings = serde_yaml::from_str(&contents)?;
        // Providers and routes come from DB, not from YAML
        settings.providers = Vec::new();
        settings.routes = Vec::new();
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.server.port, 3000);
    }

    #[test]
    fn test_default_providers_count() {
        let dp = default_providers();
        assert_eq!(dp.len(), 20);
        assert_eq!(dp[0].provider_type, "opencode_free");
        assert_eq!(dp[1].provider_type, "mimo_free");
    }

    #[test]
    fn test_default_routes_count() {
        let dr = default_routes();
        assert_eq!(dr.len(), 28);
    }

    #[test]
    fn test_rate_limit_config_default() {
        let rl = RateLimitConfig::default();
        assert!(rl.enabled);
        assert_eq!(rl.requests_per_minute, 60);
        assert_eq!(rl.burst_size, 20);
    }

    #[test]
    fn test_effective_strategy_resolution() {
        let route_fallback = RouteConfig {
            model: "test".into(), strategy: StrategyKind::Fallback,
            provider: None, providers: None, combo: None,
        };
        assert_eq!(route_fallback.effective_strategy(None), StrategyKind::Fallback);
        assert_eq!(route_fallback.effective_strategy(Some(&StrategyKind::RoundRobin)), StrategyKind::RoundRobin);
        let route_fusion = RouteConfig {
            model: "test".into(), strategy: StrategyKind::Fusion,
            provider: None, providers: None, combo: None,
        };
        assert_eq!(route_fusion.effective_strategy(Some(&StrategyKind::RoundRobin)), StrategyKind::Fusion);
    }
}
