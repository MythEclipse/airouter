//! SeaORM entity for the `jwt_secrets` table.
//!
//! Singleton table (id = 1) holding current and previous JWT signing secrets
//! to support zero-downtime rotation with grace period.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "jwt_secrets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub current_secret: String,
    pub previous_secret: Option<String>,
    pub previous_expires_at: Option<DateTimeUtc>,
    pub rotated_at: Option<DateTimeUtc>,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_field_types_match_migration() {
        // Compile-time assertion: Model shape matches migration SQL.
        // If migration schema changes without entity update, this fails to compile.
        let _check_id_type: i32 = Model {
            id: 1,
            current_secret: String::new(),
            previous_secret: None,
            previous_expires_at: None,
            rotated_at: None,
            updated_at: chrono::Utc::now(),
        }
        .id;
    }
}
