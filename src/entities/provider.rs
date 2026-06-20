use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "providers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    #[sea_orm(column_name = "provider_type")]
    pub provider_type: String,
    #[sea_orm(column_name = "api_key")]
    pub api_key: String,
    #[sea_orm(column_name = "base_url")]
    pub base_url: String,
    pub models: Vec<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub extra_headers: serde_json::Value,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
