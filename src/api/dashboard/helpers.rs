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
        _ => String::new(),
    }
}
