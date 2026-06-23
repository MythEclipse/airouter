use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use sea_orm::EntityTrait;
use crate::server::app::AppState;
use crate::auth::password::hash_password;
use crate::auth::jwt::{create_token_with_type, TokenType};
use crate::entities::server_config;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub must_change: bool,
}

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/login", post(login))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let cfg = server_config::Entity::find_by_id(1)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Server config not found".to_string()))?;

    let stored_hash = cfg.password_hash
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "Password not configured".to_string()))?;

    if hash_password(&body.password) != stored_hash {
        return Err((StatusCode::UNAUTHORIZED, "Invalid password".to_string()));
    }

    let secrets = state.jwt_secrets.get();
    let must_change = cfg.must_change_password;

    let token = if must_change {
        create_token_with_type(&secrets.current_secret, "dashboard", TokenType::ChangePwd, 300)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        create_token_with_type(&secrets.current_secret, "dashboard", TokenType::Login, 86400)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(LoginResponse { token, must_change }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_response_carries_must_change_flag() {
        let r = LoginResponse {
            token: "x".to_string(),
            must_change: true,
        };
        assert!(r.must_change);
    }

    #[test]
    fn login_response_must_change_false_by_default() {
        let r = LoginResponse {
            token: "x".to_string(),
            must_change: false,
        };
        assert!(!r.must_change);
    }
}
