use axum::{
    extract::{State, Extension},
    http::StatusCode,
    middleware::from_fn_with_state,
    response::Json,
    routing::post,
    Router,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use sea_orm::{EntityTrait, Set, ColumnTrait, QueryFilter};
use chrono::Utc;
use crate::server::app::AppState;
use crate::auth::password::{hash_password, validate_password_strength};
use crate::auth::jwt::{create_token_with_type, TokenType};
use crate::auth::middleware::auth_middleware;
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

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub new_password: String,
    pub confirm_password: String,
}

#[derive(Debug, Serialize)]
pub struct ChangePasswordResponse {
    pub token: String,
}

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let public = Router::new()
        .route("/api/auth/login", post(login));

    let protected = Router::new()
        .route("/api/auth/change-password", post(change_password))
        .route_layer(from_fn_with_state(state.clone(), auth_middleware));

    public.merge(protected)
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

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::auth::jwt::Claims>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, (StatusCode, String)> {
    // Enforce token type (defense in depth)
    if claims.typ != TokenType::ChangePwd {
        return Err((StatusCode::FORBIDDEN, "Wrong token type".to_string()));
    }
    // Validate inputs
    if req.new_password != req.confirm_password {
        return Err((StatusCode::BAD_REQUEST, "Passwords do not match".to_string()));
    }
    validate_password_strength(&req.new_password)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    // Update DB
    let new_hash = hash_password(&req.new_password);
    let now = Utc::now();
    server_config::Entity::update_many()
        .set(server_config::ActiveModel {
            password_hash: Set(Some(new_hash)),
            password_changed_at: Set(Some(now)),
            must_change_password: Set(false),
            ..Default::default()
        })
        .filter(server_config::Column::Id.eq(1))
        .exec(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Issue regular login token
    let secrets = state.jwt_secrets.get();
    let token = create_token_with_type(
        &secrets.current_secret,
        "dashboard",
        TokenType::Login,
        86400,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tracing::info!("Admin password changed");
    Ok(Json(ChangePasswordResponse { token }))
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

    #[test]
    fn change_password_requires_matching_confirmation() {
        let r = ChangePasswordRequest {
            new_password: "abc".to_string(),
            confirm_password: "xyz".to_string(),
        };
        assert_ne!(r.new_password, r.confirm_password);
    }
}
