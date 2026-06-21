use serde::{Serialize, Deserialize};

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
