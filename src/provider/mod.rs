// ─── Provider Implementations ────────────────────────────────────
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
pub mod grok_web;
pub mod perplexity_web;

// ─── Provider Framework ─────────────────────────────────────────
pub mod category;
pub mod trait_def;
pub mod registry;

// ─── Re-exports ─────────────────────────────────────────────────
// All public items re-exported at crate::provider::* for backward compat.
pub use category::{
    ProviderCategory, KNOWN_PROVIDER_TYPES,
    category_for_type, category_to_str, is_free_type,
};
pub use trait_def::{Provider, ProviderError, ProviderStream, ErrorClass};
pub use registry::ProviderRegistry;
