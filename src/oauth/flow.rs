/// Pure async OAuth flow functions: PKCE, device code, token management.
/// Uses reqwest for HTTP calls — no axum dependency.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::providers::ProviderOAuthConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

// ─── Helpers ─────────────────────────────────────────────────────

/// Encode bytes as base64url (no padding).
fn base64url_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((data.len() * 4 + 2) / 3);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        }
    }
    out
}

/// Simple percent-encoding for URL query values (RFC 3986 unreserved).
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", byte));
            }
        }
    }
    out
}

// ─── Public API ──────────────────────────────────────────────────

/// Generate a PKCE code verifier and S256 code challenge.
/// Returns `(code_verifier, code_challenge)`.
pub fn generate_pkce_pair() -> (String, String) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let code_verifier: String = (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..64u8);
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
            CHARS[idx as usize] as char
        })
        .collect();

    let digest = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = base64url_encode(&digest);

    (code_verifier, code_challenge)
}

/// Build the full authorization URL with query parameters for Authorization Code + PKCE.
pub fn build_authorize_url(
    config: &ProviderOAuthConfig,
    state: &str,
    challenge: &str,
    redirect_uri: &str,
) -> String {
    let scopes = config.scopes.join(" ");

    let params = [
        ("response_type", "code"),
        ("client_id", &config.client_id),
        ("redirect_uri", redirect_uri),
        ("scope", &scopes),
        ("state", state),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
    ];

    let query: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{}", config.auth_url, query)
}

/// Exchange an authorization code for tokens (Authorization Code + PKCE).
pub async fn exchange_auth_code(
    config: &ProviderOAuthConfig,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<OAuthTokenResponse, anyhow::Error> {
    let client = Client::new();
    let scopes = config.scopes.join(" ");

    let mut params: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", &config.client_id),
        ("code_verifier", verifier),
    ];

    if !scopes.is_empty() {
        params.push(("scope", &scopes));
    }

    let resp = client.post(&config.token_url).form(&params).send().await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        // Some providers return error in JSON body instead of HTTP error
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                anyhow::bail!("Token exchange failed: {} — {:?}", err, v.get("error_description"));
            }
        }
        anyhow::bail!("Token exchange failed (HTTP {}): {}", status, body);
    }

    let token: OAuthTokenResponse = serde_json::from_str(&body)?;
    Ok(token)
}

/// Request a device code for device-authorization flow.
pub async fn request_device_code(
    config: &ProviderOAuthConfig,
) -> Result<DeviceCodeResponse, anyhow::Error> {
    let client = Client::new();
    let scopes = config.scopes.join(" ");

    let mut params: Vec<(&str, &str)> = vec![("client_id", &config.client_id)];
    if !scopes.is_empty() {
        params.push(("scope", &scopes));
    }

    let endpoint = if config.device_code_url.is_empty() {
        // Default GitHub-style device code endpoint
        "https://github.com/login/device/code"
    } else {
        config.device_code_url.as_str()
    };

    let resp = client.post(endpoint).form(&params).send().await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("Device code request failed (HTTP {}): {}", status, body);
    }

    let device: DeviceCodeResponse = serde_json::from_str(&body)?;
    Ok(device)
}

/// Poll the token endpoint with a device code.
/// Returns the token on success, or an error with a known message for pending/expired.
pub async fn poll_device_token(
    config: &ProviderOAuthConfig,
    device_code: &str,
) -> Result<OAuthTokenResponse, anyhow::Error> {
    let client = Client::new();

    let params: Vec<(&str, &str)> = vec![
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("device_code", device_code),
        ("client_id", &config.client_id),
    ];

    let token_url = if config.device_token_url().is_empty() || config.device_token_url() == config.token_url {
        &config.token_url
    } else {
        config.device_token_url()
    };

    let resp = client.post(token_url).form(&params).send().await?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await?;

    // Handle provider-specific error responses
    if let Some(error) = body.get("error").and_then(|v| v.as_str()) {
        match error {
            "authorization_pending" | "slow_down" => {
                anyhow::bail!("authorization_pending");
            }
            "expired_token" => {
                anyhow::bail!("expired_token");
            }
            other => {
                anyhow::bail!(
                    "Device token poll error: {} — {}",
                    other,
                    body.get("error_description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                );
            }
        }
    }

    if !status.is_success() {
        anyhow::bail!("Token poll failed (HTTP {}): {}", status, body);
    }

    let token: OAuthTokenResponse = serde_json::from_value(body)?;
    Ok(token)
}

/// Refresh an expired access token.
pub async fn refresh_token(
    config: &ProviderOAuthConfig,
    refresh_token_val: &str,
) -> Result<OAuthTokenResponse, anyhow::Error> {
    let client = Client::new();
    let scopes = config.scopes.join(" ");

    let mut params: Vec<(&str, &str)> = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token_val),
        ("client_id", &config.client_id),
    ];

    if !scopes.is_empty() {
        params.push(("scope", &scopes));
    }

    let resp = client.post(&config.token_url).form(&params).send().await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        anyhow::bail!("Token refresh failed (HTTP {}): {}", status, body);
    }

    let token: OAuthTokenResponse = serde_json::from_str(&body)?;
    Ok(token)
}
