//! JWT secret store backed by Postgres with in-memory ArcSwap cache.
//!
//! All instances read from the same Postgres row, so they all see the same
//! current and previous secrets. A background task refreshes the cache
//! every 5 minutes so rotation propagates to all instances.

use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::{Deserialize, Serialize};

use crate::entities::jwt_secret;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JwtSecrets {
    pub current_secret: String,
    pub previous_secret: Option<String>,
    pub previous_expires_at: Option<DateTime<Utc>>,
}

pub struct JwtSecretStore {
    db: DatabaseConnection,
    cache: Arc<ArcSwap<JwtSecrets>>,
}

impl JwtSecretStore {
    /// Create new store and load initial value from Postgres.
    pub async fn new(db: DatabaseConnection) -> anyhow::Result<Arc<Self>> {
        let store = Arc::new(Self {
            db,
            cache: Arc::new(ArcSwap::from_pointee(JwtSecrets {
                current_secret: String::new(),
                previous_secret: None,
                previous_expires_at: None,
            })),
        });
        store.refresh_from_db().await?;
        Ok(store)
    }

    /// Get current cached snapshot (lock-free read).
    pub fn get(&self) -> JwtSecrets {
        self.cache.load().as_ref().clone()
    }

    /// Refresh cache from Postgres. Called at startup and every 5 min.
    pub async fn refresh_from_db(&self) -> anyhow::Result<()> {
        let row = jwt_secret::Entity::find_by_id(1)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("jwt_secrets row missing (id=1)"))?;

        let new_value = JwtSecrets {
            current_secret: row.current_secret,
            previous_secret: row.previous_secret,
            previous_expires_at: row.previous_expires_at,
        };
        self.cache.store(Arc::new(new_value));
        Ok(())
    }

    /// Spawn background task that refreshes cache every 5 minutes.
    pub fn spawn_refresh_task(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let store = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                if let Err(e) = store.refresh_from_db().await {
                    tracing::warn!(error = %e, "JWT secret cache refresh failed");
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_secrets_struct_carries_required_fields() {
        let s = JwtSecrets {
            current_secret: "abc".to_string(),
            previous_secret: Some("xyz".to_string()),
            previous_expires_at: Some(Utc::now()),
        };
        assert_eq!(s.current_secret, "abc");
        assert_eq!(s.previous_secret.as_deref(), Some("xyz"));
        assert!(s.previous_expires_at.is_some());
    }

    #[test]
    fn jwt_secrets_struct_allows_null_previous() {
        let s = JwtSecrets {
            current_secret: "abc".to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        assert!(s.previous_secret.is_none());
        assert!(s.previous_expires_at.is_none());
    }
}
