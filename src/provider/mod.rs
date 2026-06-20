pub mod openai;
pub mod anthropic;
pub mod openai_compat;
pub mod opencode_free;
pub mod mimo_free;
pub mod groq;
pub mod deepseek;
pub mod gemini;
pub mod openrouter;
pub mod ollama;

use std::collections::HashMap;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use crate::config::settings::ProviderConfig;
use crate::types::openai::{ChatCompletionRequest, ChatCompletionResponse, ChatCompletionChunk, ModelListResponse};

// ─── Provider Category ───────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderCategory {
    Free,
    #[serde(rename = "free-tier")]
    FreeTier,
    #[serde(rename = "api-key")]
    ApiKey,
    Oauth,
    #[serde(rename = "web-cookie")]
    WebCookie,
}

impl ProviderCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Free => "Free (No Key)",
            Self::FreeTier => "Free Tier",
            Self::ApiKey => "API Key",
            Self::Oauth => "OAuth",
            Self::WebCookie => "Web Cookie",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::Free => "#2da44e",
            Self::FreeTier => "#9a6bdb",
            Self::ApiKey => "#58a6ff",
            Self::Oauth => "#db6b28",
            Self::WebCookie => "#db6b9a",
        }
    }

    pub fn needs_api_key(&self) -> bool {
        matches!(self, Self::FreeTier | Self::ApiKey)
    }

    pub fn no_auth(&self) -> bool {
        matches!(self, Self::Free)
    }
}

/// Known provider types with display name and category.
/// Single source of truth for backend validation + frontend dropdown.
pub const KNOWN_PROVIDER_TYPES: &[(&str, &str, ProviderCategory)] = &[
    // ── Free (no auth needed) ────────────────────────────────────
    ("opencode_free",   "OpenCode Free",            ProviderCategory::Free),
    ("mimo_free",       "MiMo Free",                ProviderCategory::Free),
    // ── Free Tier (signup required, free usage) ──────────────────
    ("gemini",          "Google Gemini (AI Studio)", ProviderCategory::FreeTier),
    ("groq",            "Groq",                      ProviderCategory::FreeTier),
    ("mistral",         "Mistral AI",                ProviderCategory::FreeTier),
    ("cloudflare",      "Cloudflare Workers AI",     ProviderCategory::FreeTier),
    ("replicate",       "Replicate",                 ProviderCategory::FreeTier),
    ("novita",          "Novita AI",                 ProviderCategory::FreeTier),
    // ── API Key (paid) ───────────────────────────────────────────
    ("openai",          "OpenAI",                    ProviderCategory::ApiKey),
    ("anthropic",       "Anthropic",                 ProviderCategory::ApiKey),
    ("deepseek",        "DeepSeek",                  ProviderCategory::ApiKey),
    ("openrouter",      "OpenRouter",                ProviderCategory::ApiKey),
    ("azure_openai",    "Azure OpenAI",              ProviderCategory::ApiKey),
    ("together",        "Together AI",               ProviderCategory::ApiKey),
    ("fireworks",       "Fireworks AI",              ProviderCategory::ApiKey),
    ("openai_compat",   "OpenAI Compatible",         ProviderCategory::ApiKey),
    ("ollama",          "Ollama (Local)",            ProviderCategory::ApiKey),
    ("cohere",          "Cohere",                    ProviderCategory::ApiKey),
    ("perplexity",      "Perplexity AI",             ProviderCategory::ApiKey),
    ("xai",             "xAI (Grok)",                ProviderCategory::ApiKey),
];

/// Look up the category for a provider type string.
pub fn category_for_type(provider_type: &str) -> Option<ProviderCategory> {
    KNOWN_PROVIDER_TYPES.iter()
        .find(|(t, _, _)| *t == provider_type)
        .map(|(_, _, cat)| *cat)
}

/// Serialize a category to its API string representation.
pub fn category_to_str(cat: ProviderCategory) -> &'static str {
    match cat {
        ProviderCategory::Free => "free",
        ProviderCategory::FreeTier => "free-tier",
        ProviderCategory::ApiKey => "api-key",
        ProviderCategory::Oauth => "oauth",
        ProviderCategory::WebCookie => "web-cookie",
    }
}

/// Returns true if the provider type requires no authentication whatsoever.
pub fn is_free_type(provider_type: &str) -> bool {
    matches!(category_for_type(provider_type), Some(ProviderCategory::Free))
}

// ─── Provider Trait ──────────────────────────────────────────────

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_type(&self) -> &str;
    fn models(&self) -> &[String];

    async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError>;

    async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ProviderStream, ProviderError>;

    async fn list_models(&self) -> Result<ModelListResponse, ProviderError>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorClass {
    RateLimited,    // 429 -> double cooldown
    BadRequest,     // 4xx non-429 -> stop chain
    ServerError,    // 5xx -> standard cooldown
    Transient,      // network, timeout, unavailable -> retryable
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error ({status}): {body}")]
    Api { status: u16, body: String },
    #[error("Stream error: {0}")]
    Stream(String),
    #[error("Provider unavailable: {0}")]
    Unavailable(String),
}

impl ProviderError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, ProviderError::Http(_) | ProviderError::Unavailable(_))
    }

    pub fn error_class(&self) -> ErrorClass {
        match self {
            ProviderError::Api { status: 429, .. } => ErrorClass::RateLimited,
            ProviderError::Api { status, .. } if *status >= 400 && *status < 500 => ErrorClass::BadRequest,
            ProviderError::Api { .. } => ErrorClass::ServerError,
            ProviderError::Http(_) | ProviderError::Unavailable(_) => ErrorClass::Transient,
            ProviderError::Stream(_) => ErrorClass::Transient,
        }
    }
}

pub type ProviderStream =
    std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send>>;

// ─── ProviderRegistry ────────────────────────────────────────────

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
    pub capabilities: HashMap<String, Vec<String>>,
    pub categories: HashMap<String, ProviderCategory>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: HashMap::new(), capabilities: HashMap::new(), categories: HashMap::new() }
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Provider>> {
        self.providers.get(name)
    }

    pub fn all(&self) -> impl Iterator<Item = &Box<dyn Provider>> {
        self.providers.values()
    }

    pub fn has_capability(&self, name: &str, capability: &str) -> bool {
        self.capabilities
            .get(name)
            .map(|caps| caps.iter().any(|c| c == capability))
            .unwrap_or(false)
    }

    /// Get the category for a registered provider by name.
    pub fn category_of(&self, name: &str) -> Option<ProviderCategory> {
        self.categories.get(name).copied()
    }

    pub fn from_config(configs: &[ProviderConfig]) -> Self {
        let mut registry = Self::new();
        for cfg in configs {
            // Store capabilities
            if !cfg.capabilities.is_empty() {
                registry.capabilities.insert(cfg.name.clone(), cfg.capabilities.clone());
            }

            // Store category
            if let Some(cat) = category_for_type(&cfg.provider_type) {
                registry.categories.insert(cfg.name.clone(), cat);
            }

            let provider: Box<dyn Provider> = match cfg.provider_type.as_str() {
                "openai" => Box::new(openai::OpenAIProvider::new(cfg)),
                "anthropic" => Box::new(anthropic::AnthropicProvider::new(cfg)),
                "opencode_free" => Box::new(opencode_free::OpenCodeFreeProvider::new(cfg)),
                "mimo_free" => Box::new(mimo_free::MimoFreeProvider::new(cfg)),
                "openai_compat" => Box::new(openai_compat::OpenAICompatProvider::new(cfg)),
                "groq" => Box::new(groq::GroqProvider::new(cfg)),
                "deepseek" => Box::new(deepseek::DeepSeekProvider::new(cfg)),
                "gemini" => Box::new(gemini::GeminiProvider::new(cfg)),
                "openrouter" => Box::new(openrouter::OpenRouterProvider::new(cfg)),
                "ollama" => Box::new(ollama::OllamaProvider::new(cfg)),
                other => {
                    tracing::warn!("Unknown provider type: {}, falling back to openai_compat", other);
                    Box::new(openai_compat::OpenAICompatProvider::new(cfg))
                }
            };
            registry.register(provider);
        }
        registry
    }
}
