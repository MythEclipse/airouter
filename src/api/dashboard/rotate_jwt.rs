//! Endpoint to rotate the JWT signing secret with grace period.
//!
//! Moves current secret to previous with a configurable grace period (1-168 hours),
//! generates a new current secret, and refreshes the local cache immediately.
//! Other instances pick up the new secret within 5 minutes via the background poll.

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json};
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::entities::jwt_secret;
use crate::server::app::AppState;

#[derive(Debug, Deserialize)]
pub struct RotateRequest {
    #[serde(default = "default_grace")]
    pub grace_period_hours: i64,
}

fn default_grace() -> i64 {
    24
}

#[derive(Debug, Serialize)]
pub struct RotateResponse {
    pub previous_expires_at: chrono::DateTime<Utc>,
    pub rotated_at: chrono::DateTime<Utc>,
    pub instances_refreshed: usize,
}

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new().route(
        "/api/dashboard/rotate-jwt-secret",
        post(rotate_jwt_secret),
    )
}

pub async fn rotate_jwt_secret(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RotateRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Cap grace period at 7 days (168 hours), minimum 1 hour
    let grace_hours = req.grace_period_hours.clamp(1, 168);
    let grace = Duration::hours(grace_hours);

    let now = Utc::now();
    let previous_expires_at = now + grace;

    // Load the singleton row
    let current = jwt_secret::Entity::find_by_id(1)
        .one(&state.db)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e))
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "jwt_secrets row (id=1) not found — run seed first".to_string(),
            )
        })?;

    // Generate new 64-char hex secret
    let new_secret = generate_jwt_secret();

    // Update row: move current -> previous, set new current
    let mut active: jwt_secret::ActiveModel = current.into();
    active.previous_secret = Set(Some(active.current_secret.clone().unwrap()));
    active.previous_expires_at = Set(Some(previous_expires_at));
    active.rotated_at = Set(Some(now));
    active.current_secret = Set(new_secret);
    active.updated_at = Set(now);
    active
        .update(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Refresh this instance's cache immediately
    state
        .jwt_secrets
        .refresh_from_db()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        grace_hours = grace_hours,
        previous_expires_at = %previous_expires_at,
        "JWT secret rotated"
    );

    Ok((
        StatusCode::OK,
        Json(RotateResponse {
            previous_expires_at,
            rotated_at: now,
            instances_refreshed: 1,
        }),
    ))
}

/// Generate a 64-character hex string from 32 random bytes.
fn generate_jwt_secret() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_grace_is_24_hours() {
        assert_eq!(default_grace(), 24);
    }

    #[test]
    fn generate_jwt_secret_is_64_hex_chars() {
        let s = generate_jwt_secret();
        assert_eq!(s.len(), 64);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
