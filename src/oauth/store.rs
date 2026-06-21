/// Credential store for OAuth/WebCookie provider connections.
/// Wraps the provider_connections table.

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::provider_connection;

// ---------------------------------------------------------------------------
// OAuthConnection — simplified view used by OAuth flow handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConnection {
    pub id: Uuid,
    pub provider: String,
    pub auth_type: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Request / Response types (general-purpose CRUD)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionResponse {
    pub id: Uuid,
    pub provider_name: String,
    pub auth_type: String,
    pub display_name: String,
    pub email: Option<String>,
    pub priority: i32,
    pub is_active: bool,
    pub data: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<provider_connection::Model> for ConnectionResponse {
    fn from(m: provider_connection::Model) -> Self {
        Self {
            id: m.id,
            provider_name: m.provider_name,
            auth_type: m.auth_type,
            display_name: m.display_name,
            email: m.email,
            priority: m.priority,
            is_active: m.is_active,
            data: m.data,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateConnectionRequest {
    pub provider_name: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub data: serde_json::Value,
}

fn default_auth_type() -> String {
    "oauth".to_string()
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Save OAuth connection (used by flow handlers)
// ---------------------------------------------------------------------------

/// Save a new OAuth token connection. Packs token fields into data JSON.
pub async fn save_connection(
    db: &DatabaseConnection,
    provider: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    id_token: Option<&str>,
    expires_in: Option<u64>,
    scope: Option<&str>,
    auth_type: &str,
) -> Result<OAuthConnection, sea_orm::DbErr> {
    let data = serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "id_token": id_token,
        "expires_in": expires_in,
    });

    let now = Utc::now();
    let active = provider_connection::ActiveModel {
        id: Set(Uuid::new_v4()),
        provider_name: Set(provider.to_string()),
        auth_type: Set(auth_type.to_string()),
        display_name: Set(format!("{} connection", provider)),
        email: Set(None),
        priority: Set(0),
        is_active: Set(true),
        data: Set(data),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let model = active.insert(db).await?;

    Ok(OAuthConnection {
        id: model.id,
        provider: model.provider_name,
        auth_type: model.auth_type,
        access_token: access_token.to_string(),
        refresh_token: refresh_token.map(String::from),
        id_token: id_token.map(String::from),
        expires_in,
        scope: scope.map(String::from),
        created_at: model.created_at,
    })
}

/// Get a connection's token data by ID.
pub async fn get_connection(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<Option<OAuthConnection>, sea_orm::DbErr> {
    let model = provider_connection::Entity::find_by_id(id).one(db).await?;
    match model {
        Some(m) => {
            let access_token = m.data.get("access_token").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let refresh_token = m.data.get("refresh_token").and_then(|v| v.as_str()).map(String::from);
            let id_token = m.data.get("id_token").and_then(|v| v.as_str()).map(String::from);
            let expires_in = m.data.get("expires_in").and_then(|v| v.as_u64());

            Ok(Some(OAuthConnection {
                id: m.id,
                provider: m.provider_name,
                auth_type: m.auth_type,
                access_token,
                refresh_token,
                id_token,
                expires_in,
                scope: None,
                created_at: m.created_at,
            }))
        }
        None => Ok(None),
    }
}

/// List connections, optionally filtered by provider name.
pub async fn list_connections(
    db: &DatabaseConnection,
    provider_name: Option<&str>,
) -> Result<Vec<OAuthConnection>, sea_orm::DbErr> {
    let mut query = provider_connection::Entity::find();

    if let Some(name) = provider_name {
        query = query.filter(provider_connection::Column::ProviderName.eq(name));
    }

    let models = query.all(db).await?;
    Ok(models
        .into_iter()
        .map(|m| {
            let access_token = m
                .data
                .get("access_token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let refresh_token = m.data.get("refresh_token").and_then(|v| v.as_str()).map(String::from);
            let id_token = m.data.get("id_token").and_then(|v| v.as_str()).map(String::from);
            let expires_in = m.data.get("expires_in").and_then(|v| v.as_u64());

            OAuthConnection {
                id: m.id,
                provider: m.provider_name,
                auth_type: m.auth_type,
                access_token,
                refresh_token,
                id_token,
                expires_in,
                scope: None,
                created_at: m.created_at,
            }
        })
        .collect())
}

/// Delete a connection by ID. Returns true if deleted.
pub async fn delete_connection(
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<bool, sea_orm::DbErr> {
    let result = provider_connection::Entity::delete_by_id(id).exec(db).await?;
    Ok(result.rows_affected > 0)
}

/// Create a new connection from a generic request (for admin API).
pub async fn create_connection(
    db: &DatabaseConnection,
    req: CreateConnectionRequest,
) -> Result<ConnectionResponse, sea_orm::DbErr> {
    let now = Utc::now();
    let active = provider_connection::ActiveModel {
        id: Set(Uuid::new_v4()),
        provider_name: Set(req.provider_name),
        auth_type: Set(req.auth_type),
        display_name: Set(req.display_name),
        email: Set(req.email),
        priority: Set(req.priority),
        is_active: Set(req.is_active),
        data: Set(req.data),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let model = active.insert(db).await?;
    Ok(ConnectionResponse::from(model))
}
