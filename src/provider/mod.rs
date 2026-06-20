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
use crate::config::settings::ProviderConfig;
use crate::types::openai::{ChatCompletionRequest, ChatCompletionResponse, ChatCompletionChunk, ModelListResponse};

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

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
    pub capabilities: HashMap<String, Vec<String>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: HashMap::new(), capabilities: HashMap::new() }
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

    pub fn from_config(configs: &[ProviderConfig]) -> Self {
        let mut registry = Self::new();
        for cfg in configs {
            // Store capabilities
            if !cfg.capabilities.is_empty() {
                registry.capabilities.insert(cfg.name.clone(), cfg.capabilities.clone());
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
