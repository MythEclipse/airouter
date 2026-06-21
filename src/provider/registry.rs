use std::collections::HashMap;
use crate::config::settings::ProviderConfig;
use super::category::{ProviderCategory, category_for_type};
use super::trait_def::Provider;
use super::{openai, anthropic, openai_compat, opencode_free, mimo_free, groq, deepseek, gemini, openrouter, ollama, grok_web, perplexity_web};

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
                "grok_web" => Box::new(grok_web::GrokWebProvider::new(cfg)),
                "perplexity_web" => Box::new(perplexity_web::PerplexityWebProvider::new(cfg)),
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
