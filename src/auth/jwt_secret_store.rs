//! JWT secret store backed by Postgres with in-memory ArcSwap cache.
//!
//! All instances read from the same Postgres row, so they all see the same
//! current and previous secrets. A background task refreshes the cache
//! every 5 minutes so rotation propagates to all instances.

use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use rand::RngCore;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
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
    /// Create new store, seed row if missing, and load initial cache from Postgres.
    ///
    /// The cache is only populated after both seeding and refresh succeed, so
    /// the store is never exposed with an empty / intermediate secret.
    pub async fn new(db: DatabaseConnection) -> anyhow::Result<Arc<Self>> {
        // Defensive seeding — create the row if startup seeding missed it.
        ensure_jwt_secret_row(&db).await?;

        // Populate cache directly from DB — no empty intermediate state.
        let row = jwt_secret::Entity::find_by_id(1)
            .one(&db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("jwt_secrets row missing (id=1)"))?;

        let secrets = JwtSecrets {
            current_secret: row.current_secret,
            previous_secret: row.previous_secret,
            previous_expires_at: row.previous_expires_at,
        };

        Ok(Arc::new(Self {
            db,
            cache: Arc::new(ArcSwap::from_pointee(secrets)),
        }))
    }

    /// Get current cached snapshot (lock-free read).
    pub fn get(&self) -> JwtSecrets {
        self.cache.load().as_ref().clone()
    }

    /// Ensure the jwt_secrets row (id=1) exists. If missing, creates one
    /// with a fresh random secret. Defensive seeding so the store can
    /// self-recover even if startup seeding was missed.
    pub async fn ensure_exists(&self) -> anyhow::Result<()> {
        ensure_jwt_secret_row(&self.db).await
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

/// Create the jwt_secrets row (id=1) if it doesn't exist.
/// Generates a fresh 64-character hex secret from 32 random bytes.
async fn ensure_jwt_secret_row(db: &DatabaseConnection) -> anyhow::Result<()> {
    let existing = jwt_secret::Entity::find_by_id(1).one(db).await?;
    if existing.is_some() {
        return Ok(());
    }

    let secret = random_hex_secret();
    let now = Utc::now();
    jwt_secret::ActiveModel {
        id: Set(1),
        current_secret: Set(secret),
        previous_secret: Set(None),
        previous_expires_at: Set(None),
        rotated_at: Set(None),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Generate a 64-character hex string from 32 random bytes.
pub fn random_hex_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_hex_secret_produces_64_chars() {
        let secret = random_hex_secret();
        assert_eq!(secret.len(), 64, "secret must be 64 hex chars");
        assert!(
            secret.chars().all(|c| c.is_ascii_hexdigit()),
            "secret must be valid hex"
        );
    }

    #[test]
    fn random_hex_secret_is_random() {
        let a = random_hex_secret();
        let b = random_hex_secret();
        assert_ne!(a, b, "subsequent calls should produce different secrets");
    }

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
