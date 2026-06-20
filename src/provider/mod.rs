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
}

pub type ProviderStream =
    std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<ChatCompletionChunk, ProviderError>> + Send>>;

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: HashMap::new() }
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

    pub fn from_config(configs: &[ProviderConfig]) -> Self {
        let mut registry = Self::new();
        for cfg in configs {
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
