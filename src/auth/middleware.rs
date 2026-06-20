use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;
use crate::auth::{extract_bearer_token, sha2_hex};
use crate::server::app::AppState;

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    // Only protect API paths
    let path = req.uri().path();
    if path == "/health" || (!path.starts_with("/v1/") && !path.starts_with("/api/")) {
        return next.run(req).await;
    }

    let token = extract_bearer_token(req.headers());
    match token {
        Some(key) => {
            let hash = sha2_hex(&key);
            let hashes = state.key_hashes.load();
            if hashes.contains(&hash) {
                next.run(req).await
            } else {
                unauthorized_response()
            }
        }
        None => unauthorized_response(),
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
