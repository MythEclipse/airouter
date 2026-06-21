use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use std::sync::Arc;
use sea_orm::{EntityTrait, ActiveModelTrait, Set, ModelTrait};
use uuid::Uuid;
use std::time::Instant;
use crate::server::app::AppState;
use crate::provider::{category_for_type, category_to_str};
use super::types::*;
use super::helpers::{err_400, err_404, err_500, default_base_url};

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/dashboard/providers", get(list_providers).post(create_provider))
        .route("/api/dashboard/providers/{id}", get(get_provider).put(update_provider).delete(delete_provider))
        .route("/api/dashboard/providers/{id}/test", post(test_provider))
}

async fn list_providers(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ProviderResponse>> {
    use crate::entities::provider;
    let rows = provider::Entity::find().all(&state.db).await.unwrap_or_default();
    Json(rows.into_iter().map(row_to_provider_response).collect())
}

async fn get_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ProviderResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let row = provider::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Provider not found"))?;
    Ok(Json(row_to_provider_response(row)))
}

async fn create_provider(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<ProviderResponse>), (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;

    let base_url = if body.base_url.is_empty() {
        default_base_url(&body.provider_type)
    } else {
        body.base_url.clone()
    };

    let extra = if body.extra_headers.is_null() {
        serde_json::Value::Object(Default::default())
    } else {
        body.extra_headers
    };

    let model = provider::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(body.name),
        provider_type: Set(body.provider_type),
        api_key: Set(body.api_key),
        base_url: Set(base_url),
        models: Set(body.models),
        extra_headers: Set(extra),
        capabilities: Set(body.capabilities),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    let row = model.insert(&state.db).await
        .map_err(|e| err_400(&format!("Insert failed: {}", e)))?;
    state.reload_config().await.ok();
    Ok((StatusCode::CREATED, Json(row_to_provider_response(row))))
}

async fn update_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProviderRequest>,
) -> Result<Json<ProviderResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let existing = provider::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Provider not found"))?;

    let mut model: provider::ActiveModel = existing.into();
    if let Some(v) = body.name { model.name = Set(v); }
    if let Some(v) = body.provider_type { model.provider_type = Set(v); }
    if let Some(v) = body.api_key { model.api_key = Set(v); }
    if let Some(v) = body.base_url { model.base_url = Set(v); }
    if let Some(v) = body.models { model.models = Set(v); }
    if let Some(v) = body.capabilities { model.capabilities = Set(v); }
    if let Some(v) = body.enabled { model.enabled = Set(v); }
    model.updated_at = Set(chrono::Utc::now());

    let row = model.update(&state.db).await
        .map_err(|e| err_400(&e.to_string()))?;
    state.reload_config().await.ok();
    Ok(Json(row_to_provider_response(row)))
}

async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let existing = provider::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Provider not found"))?;
    existing.delete(&state.db).await.map_err(|_| err_500("Delete failed"))?;
    state.reload_config().await.ok();
    Ok(StatusCode::NO_CONTENT)
}

pub fn row_to_provider_response(row: crate::entities::provider::Model) -> ProviderResponse {
    let cat = category_for_type(&row.provider_type)
        .unwrap_or(crate::provider::ProviderCategory::ApiKey);
    ProviderResponse {
        id: row.id.to_string(),
        name: row.name,
        provider_type: row.provider_type,
        category: category_to_str(cat).to_string(),
        api_key: row.api_key,
        base_url: row.base_url,
        models: row.models,
        capabilities: row.capabilities,
        enabled: row.enabled,
        created_at: row.created_at.to_rfc3339(),
    }
}

async fn test_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<TestProviderRequest>,
) -> Result<Json<TestProviderResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::provider;
    let row = provider::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Provider not found"))?;

    let base_url = row.base_url.trim_end_matches('/').to_string();
    let api_key = row.api_key;
    let model = body.model;

    // Free providers have hardcoded URLs — override empty base_url
    let base_url = match row.provider_type.as_str() {
        "opencode_free" => "https://opencode.ai/zen/v1".to_string(),
        "mimo_free" => "https://api.xiaomimimo.com/api/free-ai/v1".to_string(),
        _ => {
            if base_url.is_empty() {
                return Err(err_400("Provider has no base URL configured"));
            }
            base_url
        }
    };

    // Ensure URL has a scheme
    let base_with_scheme = if base_url.starts_with("http://") || base_url.starts_with("https://") {
        base_url.clone()
    } else {
        format!("https://{}", base_url)
    };

    // Build minimal chat completion request
    let test_body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "test"}],
        "max_tokens": 1
    });

    let client = reqwest::Client::new();

    let url = format!("{}/chat/completions", base_with_scheme);
    let mut req_builder = client.post(&url)
        .header("Content-Type", "application/json")
        .json(&test_body);

    // Apply auth based on provider type
    match row.provider_type.as_str() {
        "anthropic" => {
            let an_url = format!("{}/messages", base_with_scheme);
            let an_body = serde_json::json!({
                "model": model,
                "messages": [{"role": "user", "content": "test"}],
                "max_tokens": 1
            });
            req_builder = client.post(&an_url)
                .header("Content-Type", "application/json")
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&an_body);
        }
        "opencode_free" | "mimo_free" => {
            req_builder = req_builder.header("x-opencode-client", "desktop");
        }
        _ => {
            if !api_key.is_empty() {
                req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
            }
        }
    }

    let start = Instant::now();
    match req_builder.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                Ok(Json(TestProviderResponse {
                    ok: true,
                    latency_ms: latency,
                    model: model.clone(),
                    error: None,
                }))
            } else {
                let status = resp.status().as_u16();
                let body_text = resp.text().await.unwrap_or_default();
                let err_msg = format!("HTTP {}: {}", status, body_text.chars().take(200).collect::<String>());
                Ok(Json(TestProviderResponse {
                    ok: false,
                    latency_ms: latency,
                    model: model.clone(),
                    error: Some(err_msg),
                }))
            }
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            Ok(Json(TestProviderResponse {
                ok: false,
                latency_ms: latency,
                model: model.clone(),
                error: Some(format!("Request failed: {}", e)),
            }))
        }
    }
}
