use async_trait::async_trait;
use reqwest::Client;
use sha2::{Sha256, Digest};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::config::settings::ProviderConfig;
use crate::provider::{Provider, ProviderError, ProviderStream};
use crate::types::openai::*;

/// MiMo Free — no API key required.
/// Bootstrap JWT from https://api.xiaomimimo.com/api/free-ai/bootstrap
/// Chat at https://api.xiaomimimo.com/api/free-ai/openai/chat
/// Requires system marker injection for anti-abuse gate.
pub struct MimoFreeProvider {
    name: String,
    model_list: Vec<String>,
    client: Client,
    session_id: String,
    /// Cached JWT + expiry tracking
    jwt_cache: Mutex<Option<CachedJwt>>,
}

struct CachedJwt {
    token: String,
    expires_at: Instant,
}

const BOOTSTRAP_URL: &str = "https://api.xiaomimimo.com/api/free-ai/bootstrap";
const CHAT_URL: &str = "https://api.xiaomimimo.com/api/free-ai/openai/chat";
const MIMO_SYSTEM_MARKER: &str = "You are MiMoCode, an interactive CLI tool that helps users with software engineering tasks.";
/// Refresh JWT 3 minutes before expiry (conservative 5m buffer as specified)
const JWT_REFRESH_BUFFER: Duration = Duration::from_secs(300);

fn generate_session_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    let hash = hasher.finish();
    format!("ses_{:024x}", hash)
}

fn inject_system_marker(body: &mut serde_json::Value) {
    if let Some(messages) = body.get_mut("messages").and_then(|v| v.as_array_mut()) {
        let has_marker = messages.iter().any(|m| {
            m.get("role").and_then(|r| r.as_str()) == Some("system")
                && m.get("content").and_then(|c| c.as_str()).map(|s| s.contains(MIMO_SYSTEM_MARKER)).unwrap_or(false)
        });
        if !has_marker {
            messages.insert(0, serde_json::json!({
                "role": "system",
                "content": MIMO_SYSTEM_MARKER
            }));
        }
    }
}

impl MimoFreeProvider {
    pub fn new(config: &ProviderConfig) -> Self {
        Self {
            name: config.name.clone(),
            model_list: config.models.clone(),
            client: Client::new(),
            session_id: generate_session_id(),
            jwt_cache: Mutex::new(None),
        }
    }

    fn hostname_fingerprint() -> String {
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_default();
        let platform = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let username = std::env::var("USER").unwrap_or_default();
        let raw = format!("{}|{}|{}|{}", hostname, platform, arch, username);
        let hash = hex::encode(Sha256::digest(raw.as_bytes()));
        hash
    }

    /// Get a valid JWT — uses cache when fresh, bootstrap on demand otherwise.
    async fn get_jwt(&self) -> Result<String, ProviderError> {
        // Check cache first
        {
            let cache = self.jwt_cache.lock().unwrap();
            if let Some(cached) = cache.as_ref() {
                if Instant::now() < cached.expires_at {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Bootstrap a new JWT
        let token = self.bootstrap_jwt_inner().await?;
        let cache = CachedJwt {
            token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(3600), // assume 1h lifetime
        };
        *self.jwt_cache.lock().unwrap() = Some(cache);
        Ok(token)
    }

    /// Force-refresh JWT (used on 403/rate-limit)
    async fn refresh_jwt(&self) -> Result<String, ProviderError> {
        let token = self.bootstrap_jwt_inner().await?;
        let cache = CachedJwt {
            token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        };
        *self.jwt_cache.lock().unwrap() = Some(cache);
        Ok(token)
    }

    async fn bootstrap_jwt_inner(&self) -> Result<String, ProviderError> {
        let fingerprint = Self::hostname_fingerprint();

        let resp = self.client
            .post(BOOTSTRAP_URL)
            .header("Content-Type", "application/json")
            .body(format!(r#"{{"client":"{}"}}"#, fingerprint))
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        let data: serde_json::Value = resp.json().await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        data.get("jwt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ProviderError::Http("No JWT in bootstrap response".to_string()))
    }

    /// Send a chat request to MiMo, retrying once on JWT expiry/403.
    async fn send_with_auth(&self, body: serde_json::Value) -> Result<(u16, String), ProviderError> {
        let jwt = self.get_jwt().await?;

        let resp = self.client
            .post(CHAT_URL)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", jwt))
            .header("X-Mimo-Source", "mimocode-cli-free")
            .header("x-session-affinity", &self.session_id)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        // 403 usually means expired/invalid JWT — retry once with fresh token
        if status == 403 {
            let jwt2 = self.refresh_jwt().await?;
            let retry_resp = self.client
                .post(CHAT_URL)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", jwt2))
                .header("X-Mimo-Source", "mimocode-cli-free")
                .header("x-session-affinity", &self.session_id)
                .body(body.to_string())
                .send()
                .await
                .map_err(|e| ProviderError::Http(e.to_string()))?;

            let retry_status = retry_resp.status();
            let retry_text = retry_resp.text().await.unwrap_or_default();
            return Ok((retry_status.as_u16(), retry_text));
        }

        Ok((status.as_u16(), text))
    }
}

#[async_trait]
impl Provider for MimoFreeProvider {
    fn name(&self) -> &str { &self.name }
    fn provider_type(&self) -> &str { "mimo_free" }
    fn models(&self) -> &[String] { &self.model_list }

    async fn chat_completion(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        let mut body = serde_json::to_value(&request)
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        inject_system_marker(&mut body);

        let (status, text) = self.send_with_auth(body).await?;
        if status != 200 {
            return Err(ProviderError::Api { status, body: text });
        }
        serde_json::from_str::<ChatCompletionResponse>(&text)
            .map_err(|e| ProviderError::Http(format!("JSON parse error: {}", e)))
    }

    async fn chat_completion_stream(&self, request: ChatCompletionRequest) -> Result<ProviderStream, ProviderError> {
        let mut body = serde_json::to_value(&request)
            .map_err(|e| ProviderError::Http(e.to_string()))?;
        inject_system_marker(&mut body);
        body.as_object_mut().unwrap().insert("stream".to_string(), serde_json::json!(true));

        // Force get_jwt to ensure fresh token for stream
        let jwt = self.get_jwt().await?;

        let response = self.client
            .post(CHAT_URL)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", jwt))
            .header("X-Mimo-Source", "mimocode-cli-free")
            .header("x-session-affinity", &self.session_id)
            .header("Accept", "text/event-stream")
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // Retry once on 403
            if status == 403 {
                let jwt2 = self.refresh_jwt().await?;
                let retry_resp = self.client
                    .post(CHAT_URL)
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Bearer {}", jwt2))
                    .header("X-Mimo-Source", "mimocode-cli-free")
                    .header("x-session-affinity", &self.session_id)
                    .header("Accept", "text/event-stream")
                    .body(body.clone())
                    .send()
                    .await
                    .map_err(|e| ProviderError::Http(e.to_string()))?;

                if !retry_resp.status().is_success() {
                    let s = retry_resp.status();
                    let b = retry_resp.text().await.unwrap_or_default();
                    return Err(ProviderError::Api { status: s.as_u16(), body: b });
                }

                let body_bytes = retry_resp.bytes().await.map_err(|e| ProviderError::Http(e.to_string()))?;
                return parse_sse(&body_bytes);
            }

            return Err(ProviderError::Api { status: status.as_u16(), body });
        }

        let body_bytes = response.bytes().await.map_err(|e| ProviderError::Http(e.to_string()))?;
        parse_sse(&body_bytes)
    }

    async fn list_models(&self) -> Result<ModelListResponse, ProviderError> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let data: Vec<ModelInfo> = self.model_list.iter().map(|id| ModelInfo {
            id: id.clone(),
            object: "model".to_string(),
            created: ts,
            owned_by: "mimo".to_string(),
        }).collect();
        Ok(ModelListResponse { object: "list".to_string(), data })
    }
}

fn parse_sse(body_bytes: &[u8]) -> Result<ProviderStream, ProviderError> {
    let text = String::from_utf8_lossy(body_bytes);
    let mut chunks = Vec::new();
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" { break; }
            if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                chunks.push(Ok(chunk));
            }
        }
    }
    Ok(Box::pin(futures::stream::iter(chunks)))
}
