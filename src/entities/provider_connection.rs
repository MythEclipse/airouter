use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "provider_connections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(column_name = "provider_name")]
    pub provider_name: String,
    #[sea_orm(column_name = "auth_type")]
    pub auth_type: String,
    #[sea_orm(column_name = "display_name")]
    pub display_name: String,
    pub email: Option<String>,
    pub priority: i32,
    #[sea_orm(column_name = "is_active")]
    pub is_active: bool,
    #[sea_orm(column_type = "JsonBinary")]
    pub data: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
