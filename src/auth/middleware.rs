use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use std::sync::Arc;
use crate::auth::{extract_bearer_token, validate_key};
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
        Some(key) if validate_key(&key, &state.settings.keys) => {
            next.run(req).await
        }
        _ => {
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
    }
}
