use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, delete},
};
use std::sync::Arc;
use sea_orm::{EntityTrait, ActiveModelTrait, Set, ModelTrait};
use uuid::Uuid;
use crate::server::app::AppState;
use crate::auth::sha2_hex;
use super::types::*;
use super::helpers::{err_400, err_404, err_500};

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/dashboard/api-keys", get(list_api_keys).post(create_api_key))
        .route("/api/dashboard/api-keys/{id}", delete(delete_api_key))
}

async fn list_api_keys(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ApiKeyResponse>> {
    use crate::entities::api_key;
    let rows = api_key::Entity::find().all(&state.db).await.unwrap_or_default();
    Json(rows.into_iter().map(|r| ApiKeyResponse {
        id: r.id.to_string(), key_name: r.key_name,
        key_prefix: r.key_prefix, enabled: r.enabled,
        created_at: r.created_at.to_rfc3339(),
    }).collect())
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::api_key;
    let full_key = format!("sk-{}", Uuid::new_v4().to_string().replace("-", ""));
    let prefix = full_key[..12].to_string();
    let hash = sha2_hex(&full_key);

    let model = api_key::ActiveModel {
        id: Set(Uuid::new_v4()),
        key_name: Set(body.key_name),
        key_hash: Set(hash.clone()),
        key_prefix: Set(prefix.clone()),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
    };
    let row = model.insert(&state.db).await
        .map_err(|e| err_400(&format!("Insert failed: {}", e)))?;
    state.key_store.add(&hash).await
        .map_err(|e| err_500(&format!("Failed to sync key hash to Redis: {}", e)))?;

    Ok((StatusCode::CREATED, Json(CreateApiKeyResponse {
        id: row.id.to_string(),
        key_name: row.key_name,
        key_prefix: prefix,
        full_key,
    })))
}

async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::api_key;
    let existing = api_key::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("API key not found"))?;
    let hash = existing.key_hash.clone();
    existing.delete(&state.db).await.map_err(|_| err_500("Delete failed"))?;
    state.key_store.remove(&hash).await
        .map_err(|e| err_500(&format!("Failed to remove key hash from Redis: {}", e)))?;
    Ok(StatusCode::NO_CONTENT)
}
