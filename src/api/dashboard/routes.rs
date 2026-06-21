use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::get,
};
use std::sync::Arc;
use sea_orm::{EntityTrait, ActiveModelTrait, Set, ModelTrait};
use uuid::Uuid;
use crate::server::app::AppState;
use super::types::*;
use super::helpers::{err_400, err_404, err_500};

pub fn routes() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/api/dashboard/routes", get(list_routes).post(create_route))
        .route("/api/dashboard/routes/{id}", get(get_route).put(update_route).delete(delete_route))
}

async fn list_routes(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<RouteResponse>> {
    use crate::entities::route;
    let rows = route::Entity::find().all(&state.db).await.unwrap_or_default();
    Json(rows.into_iter().map(|r| RouteResponse {
        id: r.id.to_string(), model: r.model, strategy: r.strategy,
        provider: r.provider, providers: r.providers,
        combo: r.combo, enabled: r.enabled,
    }).collect())
}

async fn get_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RouteResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;
    let row = route::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Route not found"))?;
    Ok(Json(RouteResponse {
        id: row.id.to_string(), model: row.model, strategy: row.strategy,
        provider: row.provider, providers: row.providers,
        combo: row.combo, enabled: row.enabled,
    }))
}

async fn create_route(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRouteRequest>,
) -> Result<(StatusCode, Json<RouteResponse>), (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;

    let model = route::ActiveModel {
        id: Set(Uuid::new_v4()),
        model: Set(body.model),
        strategy: Set(body.strategy),
        provider: Set(body.provider),
        providers: Set(body.providers),
        combo: Set(body.combo.unwrap_or(serde_json::Value::Null)),
        enabled: Set(true),
        created_at: Set(chrono::Utc::now()),
        updated_at: Set(chrono::Utc::now()),
    };
    let row = model.insert(&state.db).await
        .map_err(|e| err_400(&format!("Insert failed: {}", e)))?;
    state.reload_config().await.ok();
    Ok((StatusCode::CREATED, Json(RouteResponse {
        id: row.id.to_string(), model: row.model, strategy: row.strategy,
        provider: row.provider, providers: row.providers,
        combo: row.combo, enabled: row.enabled,
    })))
}

async fn update_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRouteRequest>,
) -> Result<Json<RouteResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;
    let existing = route::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Route not found"))?;

    let mut model: route::ActiveModel = existing.into();
    if let Some(v) = body.model { model.model = Set(v); }
    if let Some(v) = body.strategy { model.strategy = Set(v); }
    if let Some(v) = body.provider { model.provider = Set(v); }
    if let Some(v) = body.providers { model.providers = Set(v); }
    if let Some(v) = body.combo { model.combo = Set(v); }
    if let Some(v) = body.enabled { model.enabled = Set(v); }
    model.updated_at = Set(chrono::Utc::now());

    let row = model.update(&state.db).await
        .map_err(|e| err_400(&e.to_string()))?;
    state.reload_config().await.ok();
    Ok(Json(RouteResponse {
        id: row.id.to_string(), model: row.model, strategy: row.strategy,
        provider: row.provider, providers: row.providers,
        combo: row.combo, enabled: row.enabled,
    }))
}

async fn delete_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    use crate::entities::route;
    let existing = route::Entity::find_by_id(id).one(&state.db).await
        .map_err(|_| err_500("Database error"))?
        .ok_or_else(|| err_404("Route not found"))?;
    existing.delete(&state.db).await.map_err(|_| err_500("Delete failed"))?;
    state.reload_config().await.ok();
    Ok(StatusCode::NO_CONTENT)
}
