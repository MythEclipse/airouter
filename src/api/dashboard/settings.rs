use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::get,
};
use std::sync::Arc;
use sea_orm::EntityTrait;
use crate::server::app::AppState;
use super::types::*;
use super::helpers::{err_400, err_500};

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/dashboard/settings", get(get_settings).put(update_settings))
}

async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Json<SettingsResponse> {
    use crate::entities::{server_config, rate_limit_config};

    let sc = server_config::Entity::find_by_id(1).one(&state.db).await
        .unwrap_or(None)
        .unwrap_or(server_config::Model {
            id: 1, host: "0.0.0.0".into(), port: 3000,
            default_max_tokens: None,
            updated_at: chrono::Utc::now(),
        });

    let rl = rate_limit_config::Entity::find_by_id(1).one(&state.db).await
        .unwrap_or(None)
        .unwrap_or(rate_limit_config::Model {
            id: 1, enabled: true, requests_per_minute: 60, burst_size: 20,
            updated_at: chrono::Utc::now(),
        });

    Json(SettingsResponse {
        server: ServerSettingsResponse { host: sc.host, port: sc.port, default_max_tokens: sc.default_max_tokens },
        rate_limit: RateLimitSettingsResponse {
            enabled: rl.enabled, requests_per_minute: rl.requests_per_minute,
            burst_size: rl.burst_size,
        },
    })
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::{server_config, rate_limit_config};
    use sea_orm::ActiveModelTrait;

    if let Some(srv) = body.server {
        let existing = server_config::Entity::find_by_id(1).one(&state.db).await
            .map_err(|_| err_500("Database error"))?;
        if let Some(row) = existing {
            let mut model: server_config::ActiveModel = row.into();
            if let Some(v) = srv.host { model.host = sea_orm::Set(v); }
            if let Some(v) = srv.port { model.port = sea_orm::Set(v); }
            if let Some(v) = srv.default_max_tokens {
                let val = if v.is_some_and(|n| n > 0) { v } else { None };
                model.default_max_tokens = sea_orm::Set(val);
            }
            model.updated_at = sea_orm::Set(chrono::Utc::now());
            model.update(&state.db).await.map_err(|e| err_400(&e.to_string()))?;
        }
    }

    if let Some(rl) = body.rate_limit {
        let existing = rate_limit_config::Entity::find_by_id(1).one(&state.db).await
            .map_err(|_| err_500("Database error"))?;
        if let Some(row) = existing {
            let mut model: rate_limit_config::ActiveModel = row.into();
            if let Some(v) = rl.enabled { model.enabled = sea_orm::Set(v); }
            if let Some(v) = rl.requests_per_minute { model.requests_per_minute = sea_orm::Set(v); }
            if let Some(v) = rl.burst_size { model.burst_size = sea_orm::Set(v); }
            model.updated_at = sea_orm::Set(chrono::Utc::now());
            model.update(&state.db).await.map_err(|e| err_400(&e.to_string()))?;
        }
    }

    state.reload_config().await.ok();
    Ok(get_settings(State(state)).await)
}
