/// OAuth provider configurations.
/// Uses lazily-initialized HashMap with env-var-based client_id.

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// OAuth configuration for a single provider.
#[derive(Debug, Clone)]
pub struct ProviderOAuthConfig {
    pub display_name: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub supports_device_code: bool,
    pub device_code_url: String,
    pub device_token_url: String,
    pub supports_import: bool,
    pub is_cookie_auth: bool,
}

impl ProviderOAuthConfig {
    pub fn device_token_url(&self) -> &str {
        if self.device_token_url.is_empty() {
            &self.token_url
        } else {
            &self.device_token_url
        }
    }
}

static KNOWN_OAUTH_PROVIDERS: Lazy<HashMap<&'static str, ProviderOAuthConfig>> = Lazy::new(|| {
    let mut m = HashMap::new();

    m.insert("claude", ProviderOAuthConfig {
        display_name: "Claude".into(),
        client_id: std::env::var("CLAUDE_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: "https://anthropic.com/login/oauth/authorize".into(),
        token_url: "https://api.anthropic.com/v1/oauth/token".into(),
        scopes: vec!["openid".into(), "email".into(), "offline_access".into()],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("codex", ProviderOAuthConfig {
        display_name: "Codex".into(),
        client_id: std::env::var("CODEX_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: String::new(),
        token_url: String::new(),
        scopes: vec![],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("github", ProviderOAuthConfig {
        display_name: "GitHub".into(),
        client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: "https://github.com/login/oauth/authorize".into(),
        token_url: "https://github.com/login/oauth/access_token".into(),
        scopes: vec!["read:user".into(), "repo".into()],
        supports_device_code: true,
        device_code_url: "https://github.com/login/device/code".into(),
        device_token_url: "https://github.com/login/oauth/access_token".into(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("gitlab", ProviderOAuthConfig {
        display_name: "GitLab".into(),
        client_id: std::env::var("GITLAB_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: "https://gitlab.com/oauth/authorize".into(),
        token_url: "https://gitlab.com/oauth/token".into(),
        scopes: vec!["read_api".into(), "openid".into()],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("antigravity", ProviderOAuthConfig {
        display_name: "Antigravity".into(),
        client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
        token_url: "https://oauth2.googleapis.com/token".into(),
        scopes: vec!["openid".into(), "email".into(), "https://www.googleapis.com/auth/cloud-platform".into()],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("kilocode", ProviderOAuthConfig {
        display_name: "KiloCode".into(),
        client_id: std::env::var("KILOCODE_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: "https://github.com/login/oauth/authorize".into(),
        token_url: "https://api.kilo.ai/api/oauth/token".into(),
        scopes: vec!["read:user".into()],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("qwen", ProviderOAuthConfig {
        display_name: "Qwen".into(),
        client_id: std::env::var("QWEN_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: "https://oauth.qwen.ai/authorize".into(),
        token_url: "https://oauth.qwen.ai/token".into(),
        scopes: vec![],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("cline", ProviderOAuthConfig {
        display_name: "Cline".into(),
        client_id: std::env::var("CLINE_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: "https://github.com/login/oauth/authorize".into(),
        token_url: "https://api.cline.bot/api/oauth/token".into(),
        scopes: vec!["read:user".into()],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("cursor", ProviderOAuthConfig {
        display_name: "Cursor".into(),
        client_id: std::env::var("CURSOR_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: String::new(),
        token_url: String::new(),
        scopes: vec![],
        supports_device_code: true,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: true,
        is_cookie_auth: false,
    });

    m.insert("kimi_coding", ProviderOAuthConfig {
        display_name: "Kimi Coding".into(),
        client_id: std::env::var("KIMI_CODING_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: String::new(),
        token_url: String::new(),
        scopes: vec![],
        supports_device_code: true,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("codebuddy", ProviderOAuthConfig {
        display_name: "CodeBuddy".into(),
        client_id: std::env::var("CODEBUDDY_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: String::new(),
        token_url: String::new(),
        scopes: vec![],
        supports_device_code: true,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: false,
        is_cookie_auth: false,
    });

    m.insert("iflow", ProviderOAuthConfig {
        display_name: "iFlow".into(),
        client_id: std::env::var("IFLOW_CLIENT_ID").unwrap_or_default(),
        client_secret: None,
        auth_url: String::new(),
        token_url: String::new(),
        scopes: vec![],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: true,
        is_cookie_auth: false,
    });

    m.insert("grok_web", ProviderOAuthConfig {
        display_name: "Grok Web".into(),
        client_id: String::new(),
        client_secret: None,
        auth_url: String::new(),
        token_url: String::new(),
        scopes: vec![],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: true,
        is_cookie_auth: true,
    });

    m.insert("perplexity_web", ProviderOAuthConfig {
        display_name: "Perplexity Web".into(),
        client_id: String::new(),
        client_secret: None,
        auth_url: String::new(),
        token_url: String::new(),
        scopes: vec![],
        supports_device_code: false,
        device_code_url: String::new(),
        device_token_url: String::new(),
        supports_import: true,
        is_cookie_auth: true,
    });

    m
});

/// Look up the OAuth config for a provider by name.
pub fn get_oauth_config(provider_name: &str) -> Result<ProviderOAuthConfig, anyhow::Error> {
    KNOWN_OAUTH_PROVIDERS
        .get(provider_name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Unknown OAuth provider: {}", provider_name))
}
