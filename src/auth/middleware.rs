use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;
use crate::auth::{extract_bearer_token, sha2_hex};
use crate::auth::jwt;
use crate::server::app::AppState;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // Public paths: health, login
    if path == "/health" || path == "/api/auth/login" {
        return next.run(req).await;
    }

    // Only protect API paths
    if !path.starts_with("/v1/") && !path.starts_with("/api/") {
        return next.run(req).await;
    }

    let token = match extract_bearer_token(req.headers()) {
        Some(t) => t,
        None => return unauthorized_response(),
    };

    // Determine required scope
    let is_dashboard = path.starts_with("/api/");

    // Try JWT validation first
    let jwt_secret = state.jwt_secret.load();
    match jwt::validate_token(&token, &jwt_secret) {
        Ok(claims) => {
            // JWT valid — check scope
            let required_sub = if is_dashboard { "dashboard" } else { "ai" };
            if claims.sub == required_sub || (!is_dashboard && claims.sub == "dashboard") {
                // Dashboard token can also access AI routes, but AI token cannot access dashboard
                return next.run(req).await;
            }
            // For AI routes, dashboard token is allowed (admin accessing AI)
            // For dashboard routes, AI token is NOT allowed
            if is_dashboard && claims.sub == "ai" {
                return unauthorized_response();
            }
            unauthorized_response()
        }
        Err(_) => {
            // Not a valid JWT — fall back to legacy API key check (AI routes only)
            if is_dashboard {
                // Dashboard requires JWT (no legacy fallback for security)
                return unauthorized_response();
            }

            // AI routes: check legacy API key hash
            let hash = sha2_hex(&token);
            let hashes = state.key_hashes.load();
            if hashes.contains(&hash) {
                next.run(req).await
            } else {
                unauthorized_response()
            }
        }
    }
}

fn unauthorized_response() -> Response {
    let err = serde_json::json!({
        "error": {
            "message": "Invalid or missing API key",
            "type": "authentication_error",
            "param": null,
            "code": "invalid_api_key"
        }
    });
    (StatusCode::UNAUTHORIZED, Json(err)).into_response()
}
