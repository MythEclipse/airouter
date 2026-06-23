// ─── Shared test utilities ──────────────────────────────────────────
//
// TestEnv spins up Postgres + Redis via testcontainers.
// These tests REQUIRE Docker to be running. They will be skipped
// automatically when Docker is not available.
//
// ## Running
// ```bash
// cargo test --test security_hardening_test -- --ignored
// # or without --ignored flag once `#[ignore]` is removed
// ```

#![allow(dead_code)]

use std::sync::Arc;

use testcontainers::runners::AsyncRunner;
use airouter::auth::jwt_secret_store::JwtSecretStore;
use airouter::auth::key_store::KeyStore;
use sea_orm::DatabaseConnection;

/// Integration test environment backed by real Postgres + Redis containers.
pub struct TestEnv {
    pub db_url: String,
    pub redis_url: String,
    _postgres: testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
    _redis: testcontainers::ContainerAsync<testcontainers_modules::redis::Redis>,
}

impl TestEnv {
    /// Start Postgres and Redis containers and construct connection strings.
    ///
    /// # Panics
    /// Panics if containers fail to start (e.g. Docker not running).
    pub async fn new() -> Self {
        let pg = testcontainers_modules::postgres::Postgres::default()
            .start()
            .await
            .expect("Failed to start Postgres container (is Docker running?)");
        let redis = testcontainers_modules::redis::Redis::default()
            .start()
            .await
            .expect("Failed to start Redis container (is Docker running?)");

        let db_url = format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres",
            pg.get_host_port_ipv4(5432).await.unwrap()
        );
        let redis_url = format!(
            "redis://127.0.0.1:{}",
            redis.get_host_port_ipv4(6379).await.unwrap()
        );

        tracing::info!(db_url = %db_url, redis_url = %redis_url, "Test environment created");

        Self {
            db_url,
            redis_url,
            _postgres: pg,
            _redis: redis,
        }
    }

    /// Connect to the test Postgres database.
    pub async fn db(&self) -> DatabaseConnection {
        sea_orm::Database::connect(&self.db_url)
            .await
            .expect("Failed to connect to Postgres")
    }

    /// Create Redis connection manager and client.
    pub async fn connect_redis(&self) -> (redis::aio::ConnectionManager, redis::Client) {
        let client = redis::Client::open(self.redis_url.as_str())
            .expect("Invalid Redis URL");
        let mgr = redis::aio::ConnectionManager::new(client.clone())
            .await
            .expect("Failed to create Redis connection manager");
        (mgr, client)
    }

    /// Run all migrations and seed defaults.
    ///
    /// Returns the database connection for further use.
    pub async fn setup(&self) -> DatabaseConnection {
        let db = self.db().await;
        airouter::config::db::run_migrations(&db)
            .await
            .expect("Migrations failed");
        airouter::config::db::seed_defaults(&db)
            .await
            .expect("Seeding defaults failed");
        db
    }

    /// Create a [`JwtSecretStore`] backed by the test Postgres.
    pub async fn create_jwt_store(&self) -> Arc<JwtSecretStore> {
        let db = self.db().await;
        JwtSecretStore::new(db)
            .await
            .expect("Failed to create JwtSecretStore")
    }

    /// Create a [`KeyStore`] backed by the test Redis.
    pub async fn create_key_store(&self) -> Arc<KeyStore> {
        let (mgr, client) = self.connect_redis().await;
        KeyStore::new(mgr, client)
            .await
            .expect("Failed to create KeyStore")
    }
}
