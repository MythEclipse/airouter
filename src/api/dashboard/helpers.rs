use axum::http::StatusCode;
use axum::response::Json;

pub fn err_400(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": msg})))
}

pub fn err_404(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": msg})))
}

pub fn err_500(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": msg})))
}

/// Known default base URLs for provider types.
pub fn default_base_url(provider_type: &str) -> String {
    match provider_type {
        "openai" => "https://api.openai.com/v1".into(),
        "anthropic" => "https://api.anthropic.com/v1".into(),
        "deepseek" => "https://api.deepseek.com/v1".into(),
        "openrouter" => "https://openrouter.ai/api/v1".into(),
        "groq" => "https://api.groq.com/openai/v1".into(),
        "gemini" => "https://generativelanguage.googleapis.com/v1beta".into(),
        "ollama" => "http://localhost:11434/v1".into(),
        "together" => "https://api.together.xyz/v1".into(),
        "fireworks" => "https://api.fireworks.ai/inference/v1".into(),
        "mistral" => "https://api.mistral.ai/v1".into(),
        "cohere" => "https://api.cohere.ai/v1".into(),
        "perplexity" => "https://api.perplexity.ai".into(),
        "xai" => "https://api.x.ai/v1".into(),
        "cloudflare" => "https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/v1".into(),
        "siliconflow" => "https://api.siliconflow.com/v1".into(),
        "cerebras" => "https://api.cerebras.ai/v1".into(),
        "hyperbolic" => "https://api.hyperbolic.xyz/v1".into(),
        "nebius" => "https://api.studio.nebius.ai/v1".into(),
        "nvidia" => "https://integrate.api.nvidia.com/v1".into(),
        "chutes" => "https://llm.chutes.ai/v1".into(),
        "kimi" => "https://api.moonshot.cn/v1".into(),
        "glm_cn" => "https://open.bigmodel.cn/api/paas/v4".into(),
        "blackbox" => "https://api.blackbox.ai/v1".into(),
        // ── OAuth (login-based) ─────────────────────────────────
        "antigravity" => String::new(),
        "claude" => "https://api.anthropic.com/v1".into(),
        "cline" => "https://api.cline.bot/api/v1".into(),
        "codebuddy" => "https://copilot.tencent.com/v1".into(),
        "codex" => "https://chatgpt.com/backend-api/codex".into(),
        "cursor" => "https://api2.cursor.sh".into(),
        "github" => "https://api.githubcopilot.com".into(),
        "gitlab" => "https://gitlab.com/api/v4".into(),
        "iflow" => "https://apis.iflow.cn/v1".into(),
        "kilocode" => "https://api.kilo.ai/api".into(),
        "kimi_coding" => "https://api.kimi.com/coding/v1".into(),
        "qwen" => "https://portal.qwen.ai/v1".into(),
        // ── Web Cookie (browser session) ────────────────────────
        "grok_web" => String::new(),
        "perplexity_web" => String::new(),
        _ => String::new(),
    }
}
