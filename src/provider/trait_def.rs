use async_trait::async_trait;
use crate::types::openai::{ChatCompletionRequest, ChatCompletionResponse, ChatCompletionChunk, ModelListResponse};

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

// ─── Error Types ─────────────────────────────────────────────────

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
