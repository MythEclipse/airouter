use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::server::app::AppState;

// ─── Re-export flow types ────────────────────────────────────────

// ─── Request types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ExchangeRequest {
    pub code: String,
    pub redirect_uri: String,
    pub code_verifier: String,
}

#[derive(Debug, Deserialize)]
pub struct PollRequest {
    pub device_code: String,
}

#[derive(Debug, Deserialize)]
pub struct ListConnectionsRequest {
    pub provider: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportTokenRequest {
    pub token: String,
    pub provider: String,
}

// ─── Response types ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OAuthAuthorizeResponse {
    pub auth_url: String,
    pub state: String,
    pub code_verifier: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectionResponse {
    pub id: String,
    pub provider: String,
    pub auth_type: String,
    pub access_token_preview: String,
    pub scope: Option<String>,
    pub created_at: String,
    pub is_valid: bool,
}

#[derive(Debug, Serialize)]
pub struct PollResponse {
    pub status: String, // "pending" | "success" | "expired"
    pub connection: Option<ConnectionResponse>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub ok: bool,
    pub message: String,
}

fn err_response(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            ok: false,
            message: msg.to_string(),
        }),
    )
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "****".to_string();
    }
    format!("{}...{}", &token[..4], &token[token.len() - 4..])
}

fn to_connection_response(conn: &crate::oauth::store::OAuthConnection) -> ConnectionResponse {
    ConnectionResponse {
        id: conn.id.to_string(),
        provider: conn.provider.clone(),
        auth_type: conn.auth_type.clone(),
        access_token_preview: mask_token(&conn.access_token),
        scope: conn.scope.clone(),
        created_at: conn.created_at.to_rfc3339(),
        is_valid: true,
    }
}

// ─── Routes ──────────────────────────────────────────────────────

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Authorization Code + PKCE flow
        .route("/api/oauth/{provider}/authorize", get(authorize_handler))
        .route("/api/oauth/{provider}/exchange", post(exchange_handler))
        // Device Code flow
        .route("/api/oauth/{provider}/device-code", get(device_code_handler))
        .route("/api/oauth/{provider}/poll", post(poll_handler))
        // Token import (direct key/cookie)
        .route("/api/oauth/import-token", post(import_token_handler))
        // Connection management
        .route("/api/oauth/connections", post(list_connections_handler))
        .route("/api/oauth/connections/{id}", delete(delete_connection_handler))
        .route("/api/oauth/connections/{id}/test", post(test_connection_handler))
        .with_state(state)
}

// ─── Handlers ────────────────────────────────────────────────────

/// GET /api/oauth/{provider}/authorize
///
/// Returns an authorization URL, state nonce, and PKCE code verifier.
async fn authorize_handler(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
) -> Result<Json<OAuthAuthorizeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config = crate::oauth::providers::get_oauth_config(&provider)
        .map_err(|e| err_response(StatusCode::NOT_FOUND, &e.to_string()))?;

    let state_nonce = Uuid::new_v4().to_string();
    let (code_verifier, code_challenge) = crate::oauth::flow::generate_pkce_pair();

    let redirect_uri = std::env::var("OAUTH_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:3000/api/oauth/callback".into());

    let auth_url = crate::oauth::flow::build_authorize_url(
        &config,
        &state_nonce,
        &code_challenge,
        &redirect_uri,
    );

    Ok(Json(OAuthAuthorizeResponse {
        auth_url,
        state: state_nonce,
        code_verifier,
    }))
}

/// POST /api/oauth/{provider}/exchange
///
/// Exchanges an authorization code for tokens and stores the connection.
async fn exchange_handler(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(body): Json<ExchangeRequest>,
) -> Result<Json<ConnectionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config = crate::oauth::providers::get_oauth_config(&provider)
        .map_err(|e| err_response(StatusCode::NOT_FOUND, &e.to_string()))?;

    let redirect_uri = std::env::var("OAUTH_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:3000/api/oauth/callback".into());

    let token_resp = crate::oauth::flow::exchange_auth_code(
        &config,
        &body.code,
        &body.code_verifier,
        &redirect_uri,
    )
    .await
    .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?;

    let conn = crate::oauth::store::save_connection(
        &state.db,
        &provider,
        &token_resp.access_token,
        token_resp.refresh_token.as_deref(),
        token_resp.id_token.as_deref(),
        token_resp.expires_in,
        token_resp.scope.as_deref(),
        "oauth",
    )
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(to_connection_response(&conn)))
}

/// GET /api/oauth/{provider}/device-code
///
/// Initiates the device code flow.
async fn device_code_handler(
    State(_state): State<Arc<AppState>>,
    Path(provider): Path<String>,
) -> Result<Json<crate::oauth::flow::DeviceCodeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config = crate::oauth::providers::get_oauth_config(&provider)
        .map_err(|e| err_response(StatusCode::NOT_FOUND, &e.to_string()))?;

    let device = crate::oauth::flow::request_device_code(&config)
        .await
        .map_err(|e| err_response(StatusCode::BAD_GATEWAY, &e.to_string()))?;

    Ok(Json(device))
}

/// POST /api/oauth/{provider}/poll
///
/// Polls for a device code token. Returns status: "pending" | "success" | "expired".
async fn poll_handler(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(body): Json<PollRequest>,
) -> Result<Json<PollResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config = crate::oauth::providers::get_oauth_config(&provider)
        .map_err(|e| err_response(StatusCode::NOT_FOUND, &e.to_string()))?;

    match crate::oauth::flow::poll_device_token(&config, &body.device_code).await {
        Ok(token_resp) => {
            let conn = crate::oauth::store::save_connection(
                &state.db,
                &provider,
                &token_resp.access_token,
                token_resp.refresh_token.as_deref(),
                token_resp.id_token.as_deref(),
                token_resp.expires_in,
                token_resp.scope.as_deref(),
                "device",
            )
            .await
            .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

            Ok(Json(PollResponse {
                status: "success".into(),
                connection: Some(to_connection_response(&conn)),
            }))
        }
        Err(e) => {
            let msg = e.to_string();
            let status_str = if msg.contains("expired_token") {
                "expired"
            } else {
                "pending"
            };

            Ok(Json(PollResponse {
                status: status_str.into(),
                connection: None,
            }))
        }
    }
}

/// POST /api/oauth/import-token
///
/// Directly import a bearer token (for token-import or cookie-based providers).
async fn import_token_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ImportTokenRequest>,
) -> Result<Json<ConnectionResponse>, (StatusCode, Json<ErrorResponse>)> {
    if body.token.is_empty() {
        return Err(err_response(StatusCode::BAD_REQUEST, "Token is required"));
    }

    let auth_type = if body.provider == "grok_web" || body.provider == "perplexity_web" {
        "cookie"
    } else {
        "token"
    };

    let conn = crate::oauth::store::save_connection(
        &state.db,
        &body.provider,
        &body.token,
        None, // no refresh token for direct import
        None, // no id token
        None, // unknown expiry
        None, // unknown scope
        auth_type,
    )
    .await
    .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(to_connection_response(&conn)))
}

/// POST /api/oauth/connections
///
/// List all saved OAuth connections, optionally filtered by provider.
async fn list_connections_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ListConnectionsRequest>,
) -> Result<Json<Vec<ConnectionResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let conns = crate::oauth::store::list_connections(&state.db, body.provider.as_deref())
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(conns.iter().map(to_connection_response).collect()))
}

/// DELETE /api/oauth/connections/{id}
///
/// Delete a saved OAuth connection.
async fn delete_connection_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    crate::oauth::store::delete_connection(&state.db, id)
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/oauth/connections/{id}/test
///
/// Test that a connection's token is still valid.
async fn test_connection_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let conn = crate::oauth::store::get_connection(&state.db, id)
        .await
        .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "Connection not found"))?;

    // Basic validation: check the token exists and isn't obviously empty
    let is_valid = !conn.access_token.is_empty();

    Ok(Json(serde_json::json!({
        "ok": is_valid,
        "provider": conn.provider,
        "auth_type": conn.auth_type,
        "message": if is_valid { "Token present" } else { "Token is empty" },
    })))
}
