use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::server::app::AppState;
use crate::auth::sha2_hex;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub ok: bool,
    pub dashboard_token: String,
    pub ai_token: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub ok: bool,
    pub message: String,
}

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/login", post(login))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let input_hash = sha2_hex(&body.password);
    let stored_hash = state.password_hash.load();

    if input_hash == **stored_hash {
        let secret = state.jwt_secret.load();
        let dashboard_token = crate::auth::jwt::create_dashboard_token(&secret)
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { ok: false, message: format!("JWT error: {}", e) }),
            ))?;
        let ai_token = crate::auth::jwt::create_ai_token(&secret)
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { ok: false, message: format!("JWT error: {}", e) }),
            ))?;

        Ok(Json(LoginResponse {
            ok: true,
            dashboard_token,
            ai_token,
            message: "Login successful".into(),
        }))
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse { ok: false, message: "Invalid password".into() }),
        ))
    }
}
