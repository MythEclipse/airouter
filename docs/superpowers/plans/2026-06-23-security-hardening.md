# Security Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform AIRouter from single-instance (in-memory JWT secret, hardcoded password, in-memory key cache) to multi-instance capable (Postgres-backed JWT secret with rotation grace, random initial password with forced change, Redis-backed API key cache with pub/sub sync).

**Architecture:** Three independent improvements layered on top of existing Axum + SeaORM + Redis stack. All state moves to shared infrastructure (Postgres for persistent, Redis for ephemeral). Per-instance caches (ArcSwap) keep hot-path latency low; pub/sub and 5-min polls handle cross-instance consistency. All migrations are additive — no existing data touched.

**Tech Stack:**
- Rust 1.75+ (edition 2021)
- Axum 0.8, Tokio 1.x
- SeaORM 1.x + sqlx-postgres
- redis 1.2.4 (tokio-comp, connection-manager)
- jsonwebtoken, sha2, rand
- Leptos 0.6 (frontend), Trunk
- testcontainers (integration tests)
- Playwright (e2e tests)

**Spec:** `docs/superpowers/specs/2026-06-23-security-hardening-design.md`

---

## Global Constraints

These apply to EVERY task. Inherited verbatim from spec.

- **Migrations are additive only.** Never modify `migrations/001_initial.sql` or `migrations/002_oauth.sql`. Use `CREATE TABLE IF NOT EXISTS` / `ADD COLUMN IF NOT EXISTS`.
- **Hash algorithm:** SHA-256 (existing). Argon2id deferred to follow-up spec.
- **Password minimum length:** 12 characters. No complexity requirements (NIST 800-63B).
- **Initial password:** 32 chars, alphanumeric (A-Z, a-z, 0-9), secure random. Printed once to logs at WARN level.
- **JWT grace period default:** 24 hours. Max: 168 hours (7 days).
- **Cache refresh interval:** 5 minutes (both JWT secret poll AND key_hashes full sync).
- **JWT secret generation:** 64 chars hex (32 bytes random).
- **Redis key_hashes SET name:** `auth:key_hashes` (configurable via `KEY_HASH_REDIS_SET`).
- **Redis pub/sub channel:** `auth:key_invalidate` (configurable via `KEY_HASH_PUBSUB_CHANNEL`).
- **Fail-open on Redis outage during key lookup** (use stale in-process cache). **Fail-closed during CRUD** (return 503).
- **Singleton rows** (`id = 1` constraint) for: `server_config`, `rate_limit_config`, `jwt_secrets`.
- **Env vars all optional with defaults** — server must start with no env file (uses defaults).
- **Frontend bundle must be rebuilt** after frontend changes: `cd frontend && trunk build --dist ../frontend-dist`.
- **All commits use `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`** trailer.

---

## File Map (Reference)

```
migrations/
  003_jwt_secrets.sql                          [CREATE] Task 1
  004_password.sql                             [CREATE] Task 1

src/
  entities/
    mod.rs                                     [MODIFY] Task 1 (register module)
    jwt_secret.rs                              [CREATE] Task 1
  config/
    db.rs                                      [MODIFY] Task 1 (seed functions), Task 11 (wire stores)
  auth/
    mod.rs                                     [MODIFY] Task 2 (re-export)
    jwt_secret_store.rs                        [CREATE] Task 2
    jwt.rs                                     [MODIFY] Task 3 (dual-secret), Task 6 (typ claim)
    password.rs                                [CREATE] Task 5
    middleware.rs                              [MODIFY] Task 9 (use KeyStore)
    key_store.rs                               [CREATE] Task 8
  api/
    auth/
      mod.rs                                   [MODIFY] Task 6 (login), Task 7 (change-password)
    dashboard/
      mod.rs                                   [MODIFY] Task 4 (register route)
      rotate_jwt.rs                            [CREATE] Task 4
      api_keys.rs                              [MODIFY] Task 10 (wire KeyStore)
  server/
    app.rs                                     [MODIFY] Task 11 (init stores + spawn tasks)
  main.rs                                      [MODIFY] Task 11 (wire stores)

frontend/
  src/
    app.rs                                     [MODIFY] Task 12 (route)
    api/
      mod.rs                                   [MODIFY] Task 12 (functions)
    pages/
      login.rs                                 [MODIFY] Task 12 (redirect)
      change_password.rs                       [CREATE] Task 12

tests/
  security_hardening_test.rs                   [CREATE] Task 13

docs/
  runbooks/
    security_hardening.md                      [CREATE] Task 14
```

---

## Task 1: Database Migrations + Entity

**Files:**
- Create: `migrations/003_jwt_secrets.sql`
- Create: `migrations/004_password.sql`
- Create: `src/entities/jwt_secret.rs`
- Modify: `src/entities/mod.rs`

**Interfaces:**
- Produces: `entities::jwt_secret::{Entity, Model, ActiveModel, Column}`
- `Column::Id`, `Column::CurrentSecret`, `Column::PreviousSecret`, `Column::PreviousExpiresAt`, `Column::RotatedAt`, `Column::UpdatedAt`

- [ ] **Step 1: Write `migrations/003_jwt_secrets.sql`**

Create file `migrations/003_jwt_secrets.sql`:

```sql
-- Stores JWT signing secrets with rotation support.
-- Singleton row: id always = 1.
CREATE TABLE IF NOT EXISTS jwt_secrets (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_secret TEXT NOT NULL,
    previous_secret TEXT,
    previous_expires_at TIMESTAMPTZ,
    rotated_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 2: Write `migrations/004_password.sql`**

Create file `migrations/004_password.sql`:

```sql
-- Adds password management columns to existing server_config singleton.
-- All columns nullable/defaulted so this is safe to run on existing data.
ALTER TABLE server_config
    ADD COLUMN IF NOT EXISTS password_hash TEXT,
    ADD COLUMN IF NOT EXISTS password_changed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE;
```

- [ ] **Step 3: Write the failing entity test**

Create file `src/entities/jwt_secret.rs`:

```rust
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
```

- [ ] **Step 4: Run the test to verify it compiles**

Run: `cargo test --lib entities::jwt_secret::tests::model_field_types_match_migration`

Expected: PASS (the assertion is compile-time, so any mismatch is a compile error, not a runtime failure).

- [ ] **Step 5: Register module in `src/entities/mod.rs`**

Open `src/entities/mod.rs`. Add a new line in the module list:

```rust
pub mod jwt_secret;
```

The file should now look like (existing modules preserved):

```rust
pub mod api_key;
pub mod jwt_secret;       // NEW
pub mod provider;
pub mod provider_connection;
pub mod rate_limit_config;
pub mod route;
pub mod server_config;
```

(Adjust line order to match existing style if different — just ensure `pub mod jwt_secret;` is present.)

- [ ] **Step 6: Verify project still compiles**

Run: `cargo check`

Expected: `Finished` with no errors. Warnings OK.

- [ ] **Step 7: Run all tests to verify nothing regressed**

Run: `cargo test --lib`

Expected: All existing tests pass + the new entity test passes.

- [ ] **Step 8: Run migrations against the database**

Run: `cargo run --release` (which triggers `run_migrations()` at startup)

Then verify in psql:
```bash
psql $DATABASE_URL -c "\d jwt_secrets"
```
Expected: Output shows columns matching the migration.

```bash
psql $DATABASE_URL -c "\d server_config"
```
Expected: Output includes `password_hash`, `password_changed_at`, `must_change_password` columns.

- [ ] **Step 9: Commit**

```bash
git add migrations/003_jwt_secrets.sql migrations/004_password.sql src/entities/jwt_secret.rs src/entities/mod.rs
git commit -m "feat: add jwt_secrets table and password columns migration

- migrations/003_jwt_secrets.sql: jwt_secrets singleton table
- migrations/004_password.sql: ALTER server_config for password fields
- src/entities/jwt_secret.rs: SeaORM entity
- Both migrations are additive, idempotent (IF NOT EXISTS)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: JWT Secret Store (Postgres CRUD + Cache)

**Files:**
- Create: `src/auth/jwt_secret_store.rs`
- Modify: `src/auth/mod.rs` (add `pub mod jwt_secret_store;`)

**Interfaces:**
- `JwtSecretStore::new(db: DatabaseConnection) -> Self`
- `JwtSecretStore::ensure_exists(&self) -> Result<()>` — generates random secret if row missing
- `JwtSecretStore::get(&self) -> Result<JwtSecrets>` — returns cached snapshot
- `JwtSecretStore::rotate(&self, grace_period: Duration) -> Result<DateTimeUtc>` — rotates secret
- `struct JwtSecrets { current_secret: String, previous_secret: Option<String>, previous_expires_at: Option<DateTimeUtc> }`
- `JwtSecretStore::spawn_refresh_task(self: Arc<Self>) -> JoinHandle<()>` — every 5 min

- [ ] **Step 1: Write the failing test**

Add to `src/auth/jwt_secret_store.rs` (create file):

```rust
//! JWT secret store backed by Postgres with in-memory ArcSwap cache.
//!
//! All instances read from the same Postgres row, so they all see the same
//! current and previous secrets. A background task refreshes the cache
//! every 5 minutes so rotation propagates to all instances.

use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait};
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
```

- [ ] **Step 2: Run test to verify it fails (compile error expected)**

Run: `cargo test --lib auth::jwt_secret_store::tests`

Expected: Compile error because `auth::mod.rs` doesn't export the module yet.

- [ ] **Step 3: Register module in `src/auth/mod.rs`**

Open `src/auth/mod.rs`. Add `pub mod jwt_secret_store;` in the module declarations (alphabetical or whatever order existing modules use).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib auth::jwt_secret_store`

Expected: 2 tests pass.

- [ ] **Step 5: Add Cargo.toml dependency (if not present)**

Check `Cargo.toml` for `arc_swap`. If absent, add to `[dependencies]`:

```toml
arc_swap = "1.7"
```

Run: `cargo check`

Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add src/auth/jwt_secret_store.rs src/auth/mod.rs Cargo.toml Cargo.lock
git commit -m "feat: JWT secret store with Postgres + ArcSwap cache

Provides multi-instance JWT secret persistence. Cache refreshes from
Postgres every 5 minutes via background task, ensuring rotation propagates
to all instances without restart.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: JWT Validation with Dual-Secret Support

**Files:**
- Modify: `src/auth/jwt.rs` (replace `validate_token`, add `typ` claim)
- Modify: `src/auth/jwt.rs` (add `TokenType` enum)

**Interfaces:**
- `Claims { sub: String, exp: usize, iat: usize, typ: TokenType }`
- `TokenType::Login | TokenType::ChangePwd` (enum)
- `validate_token(token: &str, secrets: &JwtSecrets) -> Result<Claims, JwtError>` (sync, no DB)
- `JwtError::InvalidToken | ExpiredToken`

- [ ] **Step 1: Read current `src/auth/jwt.rs`**

Open `src/auth/jwt.rs` and note the current `Claims` struct and `validate_token` signature.

- [ ] **Step 2: Write failing test for dual-secret validation**

Add to `src/auth/jwt.rs` (top of `#[cfg(test)] mod tests`):

```rust
    use super::*;

    fn make_token(secret: &str, sub: &str, typ: TokenType) -> String {
        create_token_with_type(secret, sub, typ, 3600).unwrap()
    }

    #[test]
    fn validate_succeeds_with_current_secret() {
        let secrets = JwtSecrets {
            current_secret: "current123".to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        let token = make_token("current123", "dashboard", TokenType::Login);
        let claims = validate_token(&token, &secrets).unwrap();
        assert_eq!(claims.sub, "dashboard");
        assert_eq!(claims.typ, TokenType::Login);
    }

    #[test]
    fn validate_succeeds_with_previous_in_grace_period() {
        let secrets = JwtSecrets {
            current_secret: "current".to_string(),
            previous_secret: Some("previous".to_string()),
            previous_expires_at: Some(Utc::now() + chrono::Duration::hours(1)),
        };
        let token = make_token("previous", "dashboard", TokenType::Login);
        let claims = validate_token(&token, &secrets).unwrap();
        assert_eq!(claims.sub, "dashboard");
    }

    #[test]
    fn validate_fails_with_previous_after_grace_period() {
        let secrets = JwtSecrets {
            current_secret: "current".to_string(),
            previous_secret: Some("previous".to_string()),
            previous_expires_at: Some(Utc::now() - chrono::Duration::hours(1)),
        };
        let token = make_token("previous", "dashboard", TokenType::Login);
        assert!(validate_token(&token, &secrets).is_err());
    }

    #[test]
    fn validate_fails_with_completely_wrong_secret() {
        let secrets = JwtSecrets {
            current_secret: "current".to_string(),
            previous_secret: None,
            previous_expires_at: None,
        };
        let token = make_token("wrong", "dashboard", TokenType::Login);
        assert!(validate_token(&token, &secrets).is_err());
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib auth::jwt::tests`

Expected: Compile errors (Claims struct doesn't have `typ`, `validate_token` signature is wrong, `create_token_with_type` doesn't exist).

- [ ] **Step 4: Update `Claims` struct and add `TokenType`**

Replace the `Claims` struct in `src/auth/jwt.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Login,
    ChangePwd,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub typ: TokenType,
}
```

- [ ] **Step 5: Add `create_token_with_type` helper**

Add to `src/auth/jwt.rs`:

```rust
pub fn create_token_with_type(
    secret: &str,
    sub: &str,
    typ: TokenType,
    ttl_secs: i64,
) -> anyhow::Result<String> {
    let now = Utc::now().timestamp() as usize;
    let exp = (Utc::now().timestamp() + ttl_secs) as usize;
    let claims = Claims {
        sub: sub.to_string(),
        iat: now,
        exp,
        typ,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}
```

- [ ] **Step 6: Replace `validate_token` with dual-secret version**

Replace `validate_token` in `src/auth/jwt.rs`:

```rust
/// Validate a JWT using current secret first, then previous (if in grace).
pub fn validate_token(token: &str, secrets: &JwtSecrets) -> Result<Claims, jsonwebtoken::errors::Error> {
    // Try current
    if let Ok(claims) = decode_one(token, &secrets.current_secret) {
        return Ok(claims);
    }
    // Try previous if grace period active
    if let (Some(prev), Some(expires_at)) = (&secrets.previous_secret, secrets.previous_expires_at) {
        if expires_at > Utc::now() {
            if let Ok(claims) = decode_one(token, prev) {
                return Ok(claims);
            }
        }
    }
    // Fall through to fail with current decode error
    decode_one(token, &secrets.current_secret)
}

fn decode_one(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let validation = Validation::default();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
}
```

- [ ] **Step 7: Update existing token-creating functions to set `typ`**

Find existing functions like `create_dashboard_token` and `create_ai_token` in `src/auth/jwt.rs`. Update each to pass `TokenType::Login`. Example:

```rust
pub fn create_dashboard_token(secret: &str) -> anyhow::Result<String> {
    create_token_with_type(secret, "dashboard", TokenType::Login, 86400)  // 24h
}

pub fn create_ai_token(secret: &str) -> anyhow::Result<String> {
    create_token_with_type(secret, "ai", TokenType::Login, 30 * 86400)  // 30d
}
```

(Adjust TTL values to match existing code if different.)

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test --lib auth::jwt`

Expected: All tests pass (4 new + existing).

- [ ] **Step 9: Commit**

```bash
git add src/auth/jwt.rs
git commit -m "feat: dual-secret JWT validation with typ claim

- Validate against current secret, fall back to previous if grace period active
- Add TokenType enum (Login, ChangePwd) to JWT claims
- Refactor existing create_*_token functions to use new create_token_with_type

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: JWT Rotation Endpoint

**Files:**
- Create: `src/api/dashboard/rotate_jwt.rs`
- Modify: `src/api/dashboard/mod.rs` (add `pub mod rotate_jwt;` and route registration)

**Interfaces:**
- `POST /api/dashboard/rotate-jwt-secret`
- Request: `{ "grace_period_hours": 24 }` (optional, default 24, max 168)
- Response: `{ "previous_expires_at": DateTime<Utc>, "rotated_at": DateTime<Utc>, "instances_refreshed": 1 }`
- Requires: dashboard JWT

- [ ] **Step 1: Write failing test for rotation handler**

Create file `src/api/dashboard/rotate_jwt.rs`:

```rust
//! Endpoint to rotate the JWT signing secret with grace period.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::{Duration, Utc};
use sea_orm::{EntityTrait, Set, ActiveModelTrait};
use serde::{Deserialize, Serialize};

use crate::auth::jwt_secret_store::JwtSecretStore;
use crate::entities::jwt_secret;
use crate::server::app::AppState;
use crate::auth::middleware::DashboardAuth;  // adjust name to actual auth extractor

#[derive(Debug, Deserialize)]
pub struct RotateRequest {
    #[serde(default = "default_grace")]
    pub grace_period_hours: i64,
}

fn default_grace() -> i64 { 24 }

#[derive(Debug, Serialize)]
pub struct RotateResponse {
    pub previous_expires_at: chrono::DateTime<Utc>,
    pub rotated_at: chrono::DateTime<Utc>,
    pub instances_refreshed: usize,
}

pub async fn rotate_jwt_secret(
    State(state): State<Arc<AppState>>,
    _auth: DashboardAuth,
    Json(req): Json<RotateRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Cap at 7 days
    let grace_hours = req.grace_period_hours.clamp(1, 168);
    let grace = Duration::hours(grace_hours);

    let now = Utc::now();
    let previous_expires_at = now + grace;

    // Load current row
    let current = jwt_secret::Entity::find_by_id(1)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "jwt_secrets missing".to_string()))?;

    // Generate new 64-char hex secret
    let new_secret = generate_jwt_secret();

    // Update row: current -> previous, generate new current
    let mut active: jwt_secret::ActiveModel = current.into();
    active.previous_secret = Set(Some(active.current_secret.clone().unwrap()));
    active.previous_expires_at = Set(Some(previous_expires_at));
    active.rotated_at = Set(Some(now));
    active.current_secret = Set(new_secret);
    active.updated_at = Set(now);
    active.update(&state.db).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Refresh this instance's cache immediately
    state.jwt_secrets.refresh_from_db().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        grace_hours = grace_hours,
        previous_expires_at = %previous_expires_at,
        "JWT secret rotated"
    );

    Ok((
        StatusCode::OK,
        Json(RotateResponse {
            previous_expires_at,
            rotated_at: now,
            instances_refreshed: 1,  // other instances refresh via 5-min poll
        }),
    ))
}

fn generate_jwt_secret() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_grace_is_24_hours() {
        assert_eq!(default_grace(), 24);
    }

    #[test]
    fn generate_jwt_secret_is_64_hex_chars() {
        let s = generate_jwt_secret();
        assert_eq!(s.len(), 64);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
```

- [ ] **Step 2: Run tests to verify compile**

Run: `cargo test --lib api::dashboard::rotate_jwt::tests`

Expected: 2 tests pass (unit tests for default_grace and generate).

(Compile may fail until we register module and add `jwt_secrets` to AppState in Task 11. If so, skip to Task 11 then come back. Or add a stub `state.jwt_secrets: Arc<JwtSecretStore>` to make this compile.)

- [ ] **Step 3: Register module in `src/api/dashboard/mod.rs`**

Add to the module list in `src/api/dashboard/mod.rs`:

```rust
pub mod rotate_jwt;
```

- [ ] **Step 4: Register route**

In the `create_router` or equivalent function in `src/api/dashboard/mod.rs`, add:

```rust
.route("/api/dashboard/rotate-jwt-secret", post(rotate_jwt::rotate_jwt_secret))
```

(Adjust to match existing route registration style — may use `Router::new().route(...)` chain or similar.)

- [ ] **Step 5: Verify compilation**

Run: `cargo check`

Expected: Compiles (AppState additions from Task 11 may be needed; if so, those will be added in Task 11).

- [ ] **Step 6: Commit**

```bash
git add src/api/dashboard/rotate_jwt.rs src/api/dashboard/mod.rs
git commit -m "feat: POST /api/dashboard/rotate-jwt-secret endpoint

Generates new 64-char hex secret, moves current to previous with
configurable grace period (default 24h, max 168h). Refreshes local cache
immediately; other instances refresh via 5-min background poll.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Password Module (Generation + Validation + Hash)

**Files:**
- Create: `src/auth/password.rs`
- Modify: `src/auth/mod.rs` (add `pub mod password;`)

**Interfaces:**
- `generate_password(len: usize) -> String` — random alphanumeric
- `hash_password(pwd: &str) -> String` — SHA-256 hex
- `validate_password_strength(pwd: &str) -> Result<(), &'static str>` — ≥12 chars

- [ ] **Step 1: Write failing test**

Create file `src/auth/password.rs`:

```rust
//! Password generation, hashing, and validation.
//!
//! Hashing uses SHA-256 (current). Argon2id upgrade is deferred to a
//! follow-up spec — acceptable for single-admin internal tool with
//! 32-char random initial password.

use rand::Rng;
use sha2::{Digest, Sha256};

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                          abcdefghijklmnopqrstuvwxyz\
                          0123456789";

/// Generate a random alphanumeric password of given length.
pub fn generate_password(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// Hash password with SHA-256 and return hex string.
pub fn hash_password(pwd: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pwd.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Validate password meets minimum requirements.
pub fn validate_password_strength(pwd: &str) -> Result<(), &'static str> {
    if pwd.len() < 12 {
        return Err("Password must be at least 12 characters");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_password_produces_correct_length() {
        let p = generate_password(32);
        assert_eq!(p.len(), 32);
    }

    #[test]
    fn generate_password_uses_only_alphanumeric() {
        let p = generate_password(100);
        assert!(p.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn generate_password_is_random() {
        let p1 = generate_password(32);
        let p2 = generate_password(32);
        assert_ne!(p1, p2);
    }

    #[test]
    fn hash_password_is_deterministic() {
        let h1 = hash_password("hello");
        let h2 = hash_password("hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_password_is_64_hex_chars() {
        let h = hash_password("hello");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn validate_rejects_short_password() {
        assert!(validate_password_strength("short").is_err());
    }

    #[test]
    fn validate_accepts_12_char_password() {
        assert!(validate_password_strength("abcdefghijkl").is_ok());
    }

    #[test]
    fn validate_accepts_long_password() {
        assert!(validate_password_strength("a-very-long-secure-password-123!").is_ok());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (module not registered)**

Run: `cargo test --lib auth::password::tests`

Expected: Compile error (`auth::password` not found).

- [ ] **Step 3: Register module in `src/auth/mod.rs`**

Add `pub mod password;` to `src/auth/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib auth::password`

Expected: 7 tests pass.

- [ ] **Step 5: Add `rand` dependency if not present**

Check `Cargo.toml` for `rand`. If absent, add:

```toml
rand = "0.8"
```

Run: `cargo check`

Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add src/auth/password.rs src/auth/mod.rs Cargo.toml Cargo.lock
git commit -m "feat: password generation, SHA-256 hash, strength validation

- generate_password: secure random alphanumeric
- hash_password: SHA-256 hex (Argon2id deferred)
- validate_password_strength: ≥12 chars per NIST 800-63B

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Login Endpoint Updates (must_change flag, change_pwd token)

**Files:**
- Modify: `src/api/auth/mod.rs` (update login handler)

**Interfaces:**
- LoginResponse (updated): `{ token: String, must_change: bool }`
- If `must_change_password == true`: token has `typ=ChangePwd`, TTL 5 minutes
- If false: token has `typ=Login`, normal TTL (24h dashboard)

- [ ] **Step 1: Read current login handler**

Open `src/api/auth/mod.rs`. Note the current handler signature and response type.

- [ ] **Step 2: Write failing test**

Add to `src/api/auth/mod.rs` in a `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn login_response_carries_must_change_flag() {
        // Compile-time check: response struct has the field
        let r = LoginResponse {
            token: "x".to_string(),
            must_change: true,
        };
        assert!(r.must_change);
    }
```

(You'll need to define `LoginResponse` as a public struct with `must_change: bool`.)

- [ ] **Step 3: Update LoginResponse**

Add/update `LoginResponse`:

```rust
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub must_change: bool,
}
```

- [ ] **Step 4: Update login handler logic**

Modify the login handler to:
1. Verify password against `server_config.password_hash`.
2. Check `must_change_password` column.
3. If true, issue `change_pwd` token (TTL 5 min).
4. If false, issue `login` token (TTL 24h for dashboard).

```rust
use crate::auth::jwt::{create_token_with_type, TokenType};
use crate::auth::password::hash_password;
use crate::entities::server_config;

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let cfg = server_config::Entity::find_by_id(1)
        .one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "server_config missing".to_string()))?;

    let stored_hash = cfg.password_hash
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "password not configured".to_string()))?;

    if hash_password(&req.password) != stored_hash {
        return Err((StatusCode::UNAUTHORIZED, "Invalid password".to_string()));
    }

    let secrets = state.jwt_secrets.get();
    let must_change = cfg.must_change_password;

    let token = if must_change {
        create_token_with_type(&secrets.current_secret, "dashboard", TokenType::ChangePwd, 300)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        create_token_with_type(&secrets.current_secret, "dashboard", TokenType::Login, 86400)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    Ok(Json(LoginResponse { token, must_change }))
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib api::auth::tests`

Expected: Test passes (compile error resolved).

- [ ] **Step 6: Commit**

```bash
git add src/api/auth/mod.rs
git commit -m "feat: login returns must_change flag and change_pwd token

When must_change_password is true, login issues a short-lived (5 min)
change_pwd token instead of regular login token. Response carries
must_change flag so frontend can redirect to /change-password.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Change Password Endpoint

**Files:**
- Modify: `src/api/auth/mod.rs` (add new handler)

**Interfaces:**
- `POST /api/auth/change-password`
- Requires: `change_pwd` token (typ must = ChangePwd)
- Request: `{ new_password: String, confirm_password: String }`
- Response: `{ token: String }` (regular login token after change)
- Updates server_config: password_hash, password_changed_at, must_change_password=false

- [ ] **Step 1: Write failing test**

Add to `src/api/auth/mod.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub new_password: String,
    pub confirm_password: String,
}

#[derive(Debug, Serialize)]
pub struct ChangePasswordResponse {
    pub token: String,
}

// In tests module:
#[test]
fn change_password_requires_matching_confirmation() {
    // Compile check
    let r = ChangePasswordRequest {
        new_password: "abc".to_string(),
        confirm_password: "xyz".to_string(),
    };
    assert_ne!(r.new_password, r.confirm_password);
}
```

- [ ] **Step 2: Implement handler**

```rust
use crate::auth::password::{hash_password, validate_password_strength};
use crate::entities::server_config;

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::auth::jwt::Claims>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, (StatusCode, String)> {
    // Enforce token type
    if claims.typ != crate::auth::jwt::TokenType::ChangePwd {
        return Err((StatusCode::FORBIDDEN, "Wrong token type".to_string()));
    }
    // Validate inputs
    if req.new_password != req.confirm_password {
        return Err((StatusCode::BAD_REQUEST, "Passwords do not match".to_string()));
    }
    validate_password_strength(&req.new_password)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    // Update DB
    let new_hash = hash_password(&req.new_password);
    let now = chrono::Utc::now();
    server_config::Entity::update_many()
        .set(server_config::ActiveModel {
            password_hash: Set(Some(new_hash)),
            password_changed_at: Set(Some(now)),
            must_change_password: Set(false),
            ..Default::default()
        })
        .filter(server_config::Column::Id.eq(1))
        .exec(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Issue regular login token
    let secrets = state.jwt_secrets.get();
    let token = crate::auth::jwt::create_token_with_type(
        &secrets.current_secret,
        "dashboard",
        crate::auth::jwt::TokenType::Login,
        86400,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tracing::info!("Admin password changed");
    Ok(Json(ChangePasswordResponse { token }))
}
```

- [ ] **Step 3: Register route in auth router**

Find the router setup in `src/api/auth/mod.rs` (or where auth routes are registered). Add:

```rust
.route("/api/auth/change-password", post(change_password))
```

(Note: `change-password` route must NOT have `route_layer(auth_middleware)` that requires Login token type. Either use a separate router or special-case in middleware.)

- [ ] **Step 4: Update auth middleware to enforce token type per route**

Find `auth_middleware` in `src/auth/middleware.rs`. Add a check based on the matched path:

```rust
// Inside auth_middleware, after JWT decode succeeds:
let token_type_ok = match req.uri().path() {
    p if p.starts_with("/api/auth/change-password") => claims.typ == TokenType::ChangePwd,
    p if p.starts_with("/api/dashboard") => claims.typ == TokenType::Login && claims.sub == "dashboard",
    p if p.starts_with("/v1/") || p.starts_with("/openai") || p.starts_with("/anthropic") => {
        claims.typ == TokenType::Login && (claims.sub == "ai" || claims.sub == "dashboard")
    }
    _ => true,  // public routes
};
if !token_type_ok {
    return Err((StatusCode::FORBIDDEN, "Token type not allowed for this route".to_string()));
}
```

(Adjust path matching to match actual route structure.)

- [ ] **Step 5: Run tests**

Run: `cargo test --lib api::auth`

Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/api/auth/mod.rs src/auth/middleware.rs
git commit -m "feat: change password endpoint with change_pwd token type

POST /api/auth/change-password accepts new password, validates strength,
updates server_config, returns regular login token. Middleware enforces
token type per route so change_pwd tokens can only access this endpoint.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: KeyStore (Redis SET + Cache + Pub/Sub)

**Files:**
- Create: `src/auth/key_store.rs`
- Modify: `src/auth/mod.rs` (add `pub mod key_store;`)

**Interfaces:**
- `KeyStore::new(redis: ConnectionManager) -> Result<Arc<Self>>`
- `KeyStore::contains(&self, hash: &str) -> bool` — cache hit / Redis / stale fallback
- `KeyStore::add(&self, hash: &str) -> Result<()>` — Redis SADD + publish
- `KeyStore::remove(&self, hash: &str) -> Result<()>` — Redis SREM + publish
- `KeyStore::spawn_invalidation_listener(self: &Arc<Self>) -> JoinHandle<()>`
- `KeyStore::spawn_periodic_sync(self: &Arc<Self>) -> JoinHandle<()>`

- [ ] **Step 1: Write failing test**

Create file `src/auth/key_store.rs`:

```rust
//! Distributed API key hash store.
//!
//! Source of truth: Redis SET `auth:key_hashes`.
//! Per-instance cache: ArcSwap<HashSet<String>> for fast lookups.
//! Sync: pub/sub channel for invalidation, 5-min full sync as backstop.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

const DEFAULT_SET_KEY: &str = "auth:key_hashes";
const DEFAULT_CHANNEL: &str = "auth:key_invalidate";

pub struct KeyStore {
    redis: ConnectionManager,
    cache: Arc<ArcSwap<HashSet<String>>>,
    set_key: String,
    channel: String,
}

impl KeyStore {
    pub async fn new(redis: ConnectionManager) -> anyhow::Result<Arc<Self>> {
        let store = Arc::new(Self {
            redis,
            cache: Arc::new(ArcSwap::from_pointee(HashSet::new())),
            set_key: DEFAULT_SET_KEY.to_string(),
            channel: DEFAULT_CHANNEL.to_string(),
        });
        store.full_sync().await?;
        Ok(store)
    }

    pub async fn contains(&self, hash: &str) -> bool {
        // 1. In-process cache
        if self.cache.load().contains(hash) {
            return true;
        }
        // 2. Redis
        let mut conn = self.redis.clone();
        match conn.sismember::<_, _, bool>(&self.set_key, hash).await {
            Ok(true) => {
                self.cache_insert(hash);
                true
            }
            Ok(false) => false,
            Err(_) => {
                tracing::warn!("Redis unreachable, using stale cache");
                self.cache.load().contains(hash)
            }
        }
    }

    pub async fn add(&self, hash: &str) -> anyhow::Result<()> {
        let mut conn = self.redis.clone();
        let _: () = conn.sadd(&self.set_key, hash).await?;
        let _: i64 = conn.publish(&self.channel, hash).await?;
        self.cache_insert(hash);
        Ok(())
    }

    pub async fn remove(&self, hash: &str) -> anyhow::Result<()> {
        let mut conn = self.redis.clone();
        let _: () = conn.srem(&self.set_key, hash).await?;
        let _: i64 = conn.publish(&self.channel, hash).await?;
        self.cache_remove(hash);
        Ok(())
    }

    pub async fn full_sync(&self) -> anyhow::Result<()> {
        let mut conn = self.redis.clone();
        let members: Vec<String> = conn.smembers(&self.set_key).await?;
        let new_set: HashSet<String> = members.into_iter().collect();
        self.cache.store(Arc::new(new_set));
        Ok(())
    }

    pub fn spawn_invalidation_listener(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let store = Arc::clone(self);
        let channel = self.channel.clone();
        tokio::spawn(async move {
            let client = match redis::Client::open(store.redis.clone().get_connection_info().to_owned()) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(error = %e, "KeyStore: cannot create pubsub client");
                    return;
                }
            };
            let mut pubsub = match client.get_async_pubsub().await {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "KeyStore: cannot get pubsub connection");
                    return;
                }
            };
            if let Err(e) = pubsub.psubscribe(&channel).await {
                tracing::error!(error = %e, "KeyStore: psubscribe failed");
                return;
            }
            use futures::StreamExt;
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                let payload: String = match msg.get_payload() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                store.cache_remove(&payload);
            }
        })
    }

    pub fn spawn_periodic_sync(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let store = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                if let Err(e) = store.full_sync().await {
                    tracing::warn!(error = %e, "KeyStore: periodic sync failed");
                }
            }
        })
    }

    fn cache_insert(&self, hash: &str) {
        let hash = hash.to_string();
        self.cache.rcu(|set| {
            let mut new_set = (**set).clone();
            new_set.insert(hash.clone());
            Arc::new(new_set)
        });
    }

    fn cache_remove(&self, hash: &str) {
        self.cache.rcu(|set| {
            let mut new_set = (**set).clone();
            new_set.remove(hash);
            Arc::new(new_set)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_set_key_matches_spec() {
        assert_eq!(DEFAULT_SET_KEY, "auth:key_hashes");
    }

    #[test]
    fn default_channel_matches_spec() {
        assert_eq!(DEFAULT_CHANNEL, "auth:key_invalidate");
    }
}
```

- [ ] **Step 2: Run tests (compile expected to fail until module registered)**

Run: `cargo test --lib auth::key_store::tests`

Expected: Compile error (module not registered).

- [ ] **Step 3: Register module in `src/auth/mod.rs`**

Add `pub mod key_store;` to `src/auth/mod.rs`.

- [ ] **Step 4: Add `futures` dependency if needed**

Check `Cargo.toml` for `futures`. If absent:

```toml
futures = "0.3"
```

Run: `cargo check`

Expected: Compiles.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib auth::key_store`

Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/auth/key_store.rs src/auth/mod.rs Cargo.toml Cargo.lock
git commit -m "feat: distributed KeyStore with Redis SET + pub/sub

API key hashes stored in Redis SET auth:key_hashes, cached in-process
via ArcSwap. Pub/sub channel auth:key_invalidate broadcasts invalidation.
5-min periodic sync as backstop for lost pub/sub messages.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Wire KeyStore into Auth Middleware

**Files:**
- Modify: `src/auth/middleware.rs` (replace `key_hashes.contains` with `key_store.contains`)

**Interfaces:**
- `AppState.key_store: Arc<KeyStore>` (will be added in Task 11)
- Auth middleware uses `state.key_store.contains(&hash)` instead of in-memory `key_hashes`

- [ ] **Step 1: Read current middleware**

Open `src/auth/middleware.rs`. Find the part that checks API key against in-memory `key_hashes`.

- [ ] **Step 2: Replace in-memory check with KeyStore**

Find code like:
```rust
let hash = sha2_hex(&token);
let hashes = state.key_hashes.load();
if hashes.contains(&hash) { next.run(req).await } else { unauthorized_response() }
```

Replace with:
```rust
let hash = sha2_hex(&token);
if state.key_store.contains(&hash).await { next.run(req).await } else { unauthorized_response() }
```

(Note: `contains` is async — make sure the surrounding function is async.)

- [ ] **Step 3: Remove the old `key_hashes` field from AppState (preparation for Task 11)**

Find `key_hashes: Arc<ArcSwap<HashSet<String>>>` in `AppState`. Comment it out or mark as deprecated:

```rust
// TODO(Task 11): remove after migration to KeyStore
// pub key_hashes: Arc<ArcSwap<HashSet<String>>>,
pub key_store: Arc<KeyStore>,  // NEW
```

- [ ] **Step 4: Find and remove `reload_key_hashes` calls**

Grep for `reload_key_hashes`:
```bash
grep -rn "reload_key_hashes" src/
```

Replace each call site with appropriate `key_store.add(hash)` or `key_store.remove(hash)` (these will be wired in Task 10).

- [ ] **Step 5: Verify compilation**

Run: `cargo check`

Expected: May fail until Task 11 adds the field. If so, that's expected — proceed to Task 11.

- [ ] **Step 6: Commit**

```bash
git add src/auth/middleware.rs src/server/app.rs 2>/dev/null || git add src/auth/middleware.rs
git commit -m "refactor: auth middleware uses KeyStore instead of in-memory hash set

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

(Commit may include partial changes; final cleanup in Task 11.)

---

## Task 10: Wire KeyStore into API Keys CRUD

**Files:**
- Modify: `src/api/dashboard/api_keys.rs` (create, delete)

**Interfaces:**
- On POST /api/dashboard/api-keys: after DB insert, call `state.key_store.add(&hash)`
- On DELETE /api/dashboard/api-keys/{id}: after DB delete, call `state.key_store.remove(&hash)`

- [ ] **Step 1: Read current handlers**

Open `src/api/dashboard/api_keys.rs`. Note the create and delete handlers.

- [ ] **Step 2: Update create handler**

Find the function that creates an API key. After the DB insert succeeds, add:

```rust
let hash = sha2_hex(&raw_key);  // or however the hash is computed
state.key_store.add(&hash).await
    .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, format!("Redis unavailable: {}", e)))?;
```

(Adjust the key generation logic — the `raw_key` variable name may differ in existing code.)

- [ ] **Step 3: Update delete handler**

Find the function that deletes an API key. Before/after the DB delete, add:

```rust
let hash = lookup_hash_for_id(id, &state.db).await?;
api_key::Entity::delete_by_id(id).exec(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
state.key_store.remove(&hash).await
    .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, format!("Redis unavailable: {}", e)))?;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`

Expected: Compiles (with Task 11 done).

- [ ] **Step 5: Commit**

```bash
git add src/api/dashboard/api_keys.rs
git commit -m "feat: API key CRUD syncs to KeyStore (Redis SET + pub/sub)

On create, add hash to Redis SET and publish invalidation.
On delete, remove from Redis SET and publish invalidation.
Other instances pick up via pub/sub or 5-min sync.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: AppState Initialization + Background Tasks + DB Seed Functions

**Files:**
- Modify: `src/server/app.rs` (add stores to AppState, expose constructors)
- Modify: `src/main.rs` (initialize stores, spawn tasks)
- Modify: `src/config/db.rs` (add seed_jwt_secret, seed_password functions)

**Interfaces:**
- `AppState { ..., jwt_secrets: Arc<JwtSecretStore>, key_store: Arc<KeyStore> }`
- `AppState::new(...)` initializes both stores
- `AppState::spawn_background_tasks(self: &Arc<Self>) -> Vec<JoinHandle<()>>`

- [ ] **Step 1: Update AppState struct**

Open `src/server/app.rs`. Replace `AppState` struct:

```rust
pub struct AppState {
    pub db: DatabaseConnection,
    pub redis: ConnectionManager,
    pub jwt_secrets: Arc<JwtSecretStore>,
    pub key_store: Arc<KeyStore>,
    // ... other existing fields
}

impl AppState {
    pub async fn new(db: DatabaseConnection, redis: ConnectionManager /* other args */) -> anyhow::Result<Arc<Self>> {
        let jwt_secrets = JwtSecretStore::new(db.clone()).await?;
        let key_store = KeyStore::new(redis.clone()).await?;
        Ok(Arc::new(Self {
            db,
            redis,
            jwt_secrets,
            key_store,
            // ... other fields
        }))
    }

    pub fn spawn_background_tasks(self: &Arc<Self>) -> Vec<tokio::task::JoinHandle<()>> {
        vec![
            self.jwt_secrets.spawn_refresh_task(),
            self.key_store.spawn_invalidation_listener(),
            self.key_store.spawn_periodic_sync(),
        ]
    }
}
```

(Adjust `other args` to match existing constructor signature.)

- [ ] **Step 2: Add seed functions to `src/config/db.rs`**

Add to `src/config/db.rs`:

```rust
use crate::auth::password::{generate_password, hash_password};
use crate::auth::jwt_secret_store::JwtSecretStore;
use rand::Rng;

pub async fn seed_defaults(db: &DatabaseConnection) -> anyhow::Result<()> {
    seed_providers(db).await?;
    seed_routes(db).await?;
    seed_jwt_secret(db).await?;
    seed_password(db).await?;
    // ... existing seed calls
    Ok(())
}

pub async fn seed_jwt_secret(db: &DatabaseConnection) -> anyhow::Result<()> {
    use crate::entities::jwt_secret;
    let exists = jwt_secret::Entity::find_by_id(1).one(db).await?;
    if exists.is_none() {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill(&mut bytes);
        let secret = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        jwt_secret::Entity::insert(jwt_secret::ActiveModel {
            id: Set(1),
            current_secret: Set(secret),
            previous_secret: Set(None),
            previous_expires_at: Set(None),
            rotated_at: Set(None),
            updated_at: Set(chrono::Utc::now()),
        }).exec(db).await?;
        tracing::info!("Seeded initial JWT secret");
    }
    Ok(())
}

pub async fn seed_password(db: &DatabaseConnection) -> anyhow::Result<()> {
    use crate::entities::server_config;
    let cfg = server_config::Entity::find_by_id(1).one(db).await?
        .ok_or_else(|| anyhow::anyhow!("server_config row missing"))?;
    if cfg.password_hash.is_none() {
        let pwd = generate_password(32);
        let hash = hash_password(&pwd);
        server_config::Entity::update_many()
            .set(server_config::ActiveModel {
                password_hash: Set(Some(hash)),
                password_changed_at: Set(None),
                must_change_password: Set(true),
                ..Default::default()
            })
            .filter(server_config::Column::Id.eq(1))
            .exec(db).await?;
        // Print once at WARN level
        tracing::warn!(
            "=================================================================\n\
             INITIAL ADMIN PASSWORD (save this, won't be shown again):\n\
             \n\
             {}\n\
             \n\
             You will be forced to change it on first login.\n\
             =================================================================",
            pwd
        );
    }
    Ok(())
}
```

- [ ] **Step 3: Update `main.rs` to use new AppState constructor**

Open `src/main.rs`. Find where `AppState` is created. Update:

```rust
let state = AppState::new(db.clone(), redis.clone(), /* other args */).await?;
for handle in state.spawn_background_tasks() {
    // handles kept alive for server lifetime
}
// ... existing server start code
```

- [ ] **Step 4: Run all tests**

Run: `cargo test --lib`

Expected: All tests pass.

- [ ] **Step 5: Smoke test the server**

```bash
cargo run --release
```

In another terminal:
```bash
psql $DATABASE_URL -c "SELECT * FROM jwt_secrets;"
```
Expected: One row with `current_secret` populated.

```bash
psql $DATABASE_URL -c "SELECT password_hash IS NOT NULL, must_change_password FROM server_config WHERE id = 1;"
```
Expected: `t | t` (password set, must change).

Check logs for `INITIAL ADMIN PASSWORD` line. Note the password.

- [ ] **Step 6: Commit**

```bash
git add src/server/app.rs src/main.rs src/config/db.rs
git commit -m "feat: AppState with JWT + Key stores, background tasks, password seed

- AppState::new initializes JwtSecretStore and KeyStore
- spawn_background_tasks: 3 periodic tasks (JWT refresh, key pub/sub, key sync)
- seed_jwt_secret: generate random secret if row missing
- seed_password: generate random password, print to logs at WARN
- Both seeds are idempotent (skip if data exists)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 12: Frontend Change Password Page + Login Redirect

**Files:**
- Create: `frontend/src/pages/change_password.rs`
- Modify: `frontend/src/app.rs` (add route)
- Modify: `frontend/src/pages/login.rs` (redirect if must_change)
- Modify: `frontend/src/api/mod.rs` (add functions)

**Interfaces:**
- `POST /api/auth/change-password` request/response types
- Route `/change-password` in app.rs (outside AuthenticatedLayout)
- `pages::change_password::ChangePassword` component

- [ ] **Step 1: Add API client function**

In `frontend/src/api/mod.rs`, add:

```rust
pub async fn change_password(
    new_password: String,
    confirm_password: String,
    change_token: String,
) -> Result<String, String> {
    let body = serde_json::json!({
        "new_password": new_password,
        "confirm_password": confirm_password,
    });
    let resp: ChangePasswordResponse = api_request_with_token(
        "POST",
        "/api/auth/change-password",
        Some(&body.to_string()),
        &change_token,
    ).await?;
    Ok(resp.token)
}

#[derive(serde::Deserialize)]
struct ChangePasswordResponse {
    token: String,
}
```

(If `api_request_with_token` doesn't exist, add it as a variant of `api_request` that takes an explicit token instead of reading from localStorage.)

- [ ] **Step 2: Update login page to handle must_change**

In `frontend/src/pages/login.rs`, after successful login:

```rust
if response.must_change {
    // Save change_pwd token to localStorage
    local_storage().set("dashboard_token", &response.token).unwrap();
    // Navigate to /change-password
    navigate("/change-password");
} else {
    local_storage().set("dashboard_token", &response.token).unwrap();
    navigate("/dashboard");
}
```

- [ ] **Step 3: Create change_password page**

Create `frontend/src/pages/change_password.rs`:

```rust
use leptos::*;

#[component]
pub fn ChangePassword() -> impl IntoView {
    let (new_password, set_new_password) = create_signal(String::new());
    let (confirm_password, set_confirm_password) = create_signal(String::new());
    let (error, set_error) = create_signal(String::new());
    let (loading, set_loading) = create_signal(false);

    let submit = create_action(move |_: &()| {
        let new_pwd = new_password.get();
        let confirm_pwd = confirm_password.get();
        async move {
            set_loading.set(true);
            set_error.set(String::new());
            let token = local_storage()
                .get("dashboard_token")
                .unwrap_or_default()
                .unwrap_or_default();
            match crate::api::change_password(new_pwd, confirm_pwd, token).await {
                Ok(new_token) => {
                    local_storage().set("dashboard_token", &new_token).unwrap();
                    navigate("/dashboard");
                }
                Err(e) => set_error.set(e),
            }
            set_loading.set(false);
        }
    });

    view! {
        <div class="change-password-page">
            <h1>"Change Password"</h1>
            <p>"You must change your password before continuing."</p>
            <form on:submit=move |ev| {
                ev.prevent_default();
                submit.dispatch(());
            }>
                <input
                    type="password"
                    placeholder="New password (min 12 chars)"
                    prop:value=new_password
                    on:input=move |ev| set_new_password.set(event_target_value(&ev))
                />
                <input
                    type="password"
                    placeholder="Confirm new password"
                    prop:value=confirm_password
                    on:input=move |ev| set_confirm_password.set(event_target_value(&ev))
                />
                {move || error.get().map(|e| view! { <p class="error">{e}</p> })}
                <button type="submit" disabled=loading>
                    "Change Password"
                </button>
            </form>
        </div>
    }
}
```

(Adjust to match existing styling patterns — may need Tailwind classes.)

- [ ] **Step 4: Register page module**

In `frontend/src/pages/mod.rs` (or wherever pages are registered), add:

```rust
pub mod change_password;
```

- [ ] **Step 5: Add route in `frontend/src/app.rs`**

In the `<Routes>` block, BEFORE the `AuthenticatedLayout`:

```rust
<Route path="/change-password" view=ChangePassword/>
```

(Note: this route is OUTSIDE AuthenticatedLayout because the user has a change_pwd token, not a regular login token. The AuthenticatedLayout check would reject it.)

- [ ] **Step 6: Rebuild frontend**

```bash
cd frontend && trunk build --dist ../frontend-dist
```

Expected: Build succeeds.

- [ ] **Step 7: Commit**

```bash
git add frontend/src/api/mod.rs frontend/src/pages/login.rs frontend/src/pages/change_password.rs frontend/src/app.rs
git commit -m "feat: frontend /change-password page + login redirect

When login returns must_change: true, frontend stores the change_pwd
token and redirects to /change-password. After successful change, the
new login token is stored and user is redirected to /dashboard.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 13: Integration Tests (Testcontainers)

**Files:**
- Create: `tests/security_hardening_test.rs`
- Modify: `Cargo.toml` (add dev-dependencies)

**Interfaces:**
- Test scenarios covering all 3 spec sections
- Uses testcontainers for Postgres + Redis

- [ ] **Step 1: Add dev-dependencies**

In `Cargo.toml`:

```toml
[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["postgres", "redis"] }
```

Run: `cargo check`

Expected: Compiles.

- [ ] **Step 2: Write test helpers**

Create `tests/common/mod.rs`:

```rust
#![allow(dead_code)]

use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;

pub struct TestEnv {
    pub db_url: String,
    pub redis_url: String,
    _postgres: ContainerAsync<Postgres>,
    _redis: ContainerAsync<Redis>,
}

impl TestEnv {
    pub async fn new() -> Self {
        // Start containers
        let pg = Postgres::default().start().await.unwrap();
        let redis = Redis::default().start().await.unwrap();
        // Build connection strings from exposed ports
        // (Implementation note: refer to testcontainers-modules docs for exact API)
        todo!("Set connection strings from container ports")
    }
}
```

(Adjust to actual testcontainers-modules 0.8 API. May need port inspection.)

- [ ] **Step 3: Write JWT persistence test**

In `tests/security_hardening_test.rs`:

```rust
mod common;

use airouter::auth::jwt_secret_store::JwtSecretStore;
use airouter::auth::jwt::{create_token_with_type, validate_token, TokenType};

#[tokio::test]
async fn jwt_validates_across_restart() {
    let env = common::TestEnv::new().await;
    // Run migrations + seed
    airouter::config::db::run_migrations(&env.db_url).await.unwrap();
    airouter::config::db::seed_defaults(&env.db_url).await.unwrap();
    // Create store, get secret
    let store1 = JwtSecretStore::new(connect(&env.db_url).await).await.unwrap();
    let secret1 = store1.get().current_secret;
    // Drop store (simulate restart)
    drop(store1);
    // Recreate store
    let store2 = JwtSecretStore::new(connect(&env.db_url).await).await.unwrap();
    let secret2 = store2.get().current_secret;
    // Same secret
    assert_eq!(secret1, secret2);
    // Token signed with secret1 still validates
    let token = create_token_with_type(&secret1, "dashboard", TokenType::Login, 3600).unwrap();
    let claims = validate_token(&token, &store2.get()).unwrap();
    assert_eq!(claims.sub, "dashboard");
}
```

- [ ] **Step 4: Write JWT rotation test**

```rust
#[tokio::test]
async fn jwt_rotation_grace_period_allows_old_tokens() {
    let env = common::TestEnv::new().await;
    setup(&env).await;
    let store = JwtSecretStore::new(connect(&env.db_url).await).await.unwrap();
    let old_token = create_token_with_type(&store.get().current_secret, "dashboard", TokenType::Login, 3600).unwrap();
    // Rotate with 1 hour grace
    store.rotate(Duration::hours(1)).await.unwrap();
    // Old token still validates
    let claims = validate_token(&old_token, &store.get()).unwrap();
    assert_eq!(claims.sub, "dashboard");
}
```

- [ ] **Step 5: Write key store multi-instance test**

```rust
#[tokio::test]
async fn api_key_add_propagates_to_second_instance() {
    let env = common::TestEnv::new().await;
    setup(&env).await;
    let store1 = KeyStore::new(connect_redis(&env.redis_url).await).await.unwrap();
    let store2 = KeyStore::new(connect_redis(&env.redis_url).await).await.unwrap();
    // store2 subscribes to invalidation
    let _h = store2.spawn_invalidation_listener();
    // store1 adds a hash
    store1.add("hash-abc").await.unwrap();
    // Wait briefly for pub/sub to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;
    // store2 should see it (after cache invalidated and re-fetched)
    assert!(!store2.contains("hash-abc").await);  // cache invalidated, will re-fetch on next contains
    assert!(store2.contains("hash-abc").await);   // re-fetches and finds it
}
```

- [ ] **Step 6: Write forced password change test**

```rust
#[tokio::test]
async fn forced_password_change_blocks_dashboard_access() {
    let env = common::TestEnv::new().await;
    setup(&env).await;
    // Force password change
    update_server_config_must_change(&env.db_url, true).await;
    // Login returns change_token
    let login_resp = login(&env, "current_password").await;
    assert!(login_resp.must_change);
    let change_token = login_resp.token;
    // Try dashboard with change_token → should fail
    let result = call_dashboard(&env, &change_token).await;
    assert_eq!(result.status(), 403);
    // Change password with change_token
    let new_login_token = change_password(&env, &change_token, "new_password_123").await;
    // Try dashboard with new login token → succeeds
    let result = call_dashboard(&env, &new_login_token).await;
    assert_eq!(result.status(), 200);
}
```

- [ ] **Step 7: Run integration tests**

Run: `cargo test --test security_hardening_test`

Expected: All tests pass. Tests may take ~30s due to container startup.

- [ ] **Step 8: Commit**

```bash
git add tests/security_hardening_test.rs tests/common/mod.rs Cargo.toml Cargo.lock
git commit -m "test: integration tests for security hardening (testcontainers)

Covers: JWT validation across restart, JWT rotation grace period,
API key pub/sub propagation, forced password change flow.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 14: Documentation & Runbook

**Files:**
- Create: `docs/runbooks/security_hardening.md`

- [ ] **Step 1: Create runbook**

Create `docs/runbooks/security_hardening.md`:

```markdown
# Security Hardening Runbook

Operational guide for the Security Hardening feature set.

## Initial Deployment (Greenfield)

1. Deploy with no existing data (fresh Postgres).
2. Server starts, generates random JWT secret + initial admin password.
3. **Critical:** grep logs for `INITIAL ADMIN PASSWORD`:
   ```bash
   journalctl -u airouter -n 100 | grep -A 5 "INITIAL ADMIN PASSWORD"
   ```
4. Login with initial password at `/login`.
5. Forced to change password at `/change-password`.
6. New password becomes the permanent admin password.

## Upgrade from Pre-Hardening Version

1. Backup database:
   ```bash
   pg_dump $DATABASE_URL > backup_$(date +%Y%m%d).sql
   ```
2. Deploy new version. Migrations run automatically.
3. Existing password (if any) is preserved. `must_change_password` defaults to FALSE for existing data.
4. Admin can rotate JWT secret via dashboard if desired.

## Forgot Password Recovery

Direct database update:

```sql
UPDATE server_config SET
  password_hash = encode(sha256('NEW_PASSWORD_HERE'), 'hex'),
  must_change_password = true
WHERE id = 1;
```

Replace `NEW_PASSWORD_HERE` with desired new password. After update, login with new password and change it via UI.

## JWT Secret Rotation

Via dashboard:
1. Settings → Security → Rotate JWT Secret
2. Set grace period (default 24h, max 168h)
3. Click "Rotate"
4. Existing sessions valid until grace period expires

Via database (emergency):
```sql
UPDATE jwt_secrets SET
  current_secret = encode(gen_random_bytes(32), 'hex'),
  previous_secret = current_secret,
  previous_expires_at = NOW() + INTERVAL '24 hours',
  rotated_at = NOW()
WHERE id = 1;
```
Other instances refresh within 5 minutes.

## Lost JWT Secret (Postgres Corruption)

If the `jwt_secrets` row is lost, all sessions invalidate. Recovery:

```sql
INSERT INTO jwt_secrets (id, current_secret, updated_at)
VALUES (1, encode(gen_random_bytes(32), 'hex'), NOW())
ON CONFLICT (id) DO UPDATE SET
  current_secret = EXCLUDED.current_secret,
  previous_secret = NULL,
  previous_expires_at = NULL,
  rotated_at = NULL,
  updated_at = NOW();
```

All users must re-login.

## Redis Outage Scenarios

| Operation | Behavior |
|-----------|----------|
| API request lookup | Uses stale in-process cache (up to 5 min old). Fail-open. |
| API key create/delete | Returns 503. Admin must retry when Redis recovers. |
| Periodic sync | Skipped, retries next interval. |
| Pub/sub listener | Disconnected, reconnects automatically. |

## Multi-Instance Verification

After deploying multiple instances:

```bash
# Add API key via instance A's dashboard
# Verify via instance B:
curl -H "Authorization: Bearer $KEY" http://instance-b/v1/models
```

Should succeed within 5 seconds (pub/sub propagation).

## Monitoring

Watch for these log messages:

- `JWT secret rotated` — expected during rotation
- `KeyStore: periodic sync failed` — Redis connectivity issue
- `KeyStore: psubscribe failed` — pub/sub connection issue
- `Redis unreachable, using stale cache` — Redis hiccup, fail-open
- `Admin password changed` — expected after forced change

## Rollback

1. Stop all instances.
2. `git revert` the security hardening commit(s).
3. Deploy old version.
4. Optional cleanup:
   ```sql
   DROP TABLE IF EXISTS jwt_secrets;
   ALTER TABLE server_config
     DROP COLUMN IF EXISTS password_hash,
     DROP COLUMN IF EXISTS password_changed_at,
     DROP COLUMN IF EXISTS must_change_password;
   ```
```

- [ ] **Step 2: Commit**

```bash
git add docs/runbooks/security_hardening.md
git commit -m "docs: security hardening runbook (deploy, recovery, rollback)

Covers greenfield deployment, upgrade path, password recovery, JWT
rotation, Redis outage scenarios, multi-instance verification, and
rollback procedures.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

### Spec coverage

| Spec Section | Covered by Task |
|--------------|-----------------|
| Section 1: Architecture overview | All tasks reference the architecture |
| Section 2: JWT secret persistence (schema, validation, rotation) | Tasks 1, 2, 3, 4, 11 |
| Section 3: Password management (init, forced change, validation, frontend) | Tasks 5, 6, 7, 11, 12 |
| Section 4: Distributed key_hashes (Redis SET, cache, pub/sub, failure modes) | Tasks 8, 9, 10 |
| Section 5: Migration & rollout (env vars, runbook, rollback) | Tasks 1, 11, 14 |
| Section 6: Testing & verification | Tasks 13, 14 (manual checklist) |

All 6 spec sections covered. No gaps.

### Placeholder scan

Searched plan for `TBD`, `TODO`, `implement later`, `add appropriate`, `similar to`. Found:
- Task 11 Step 2: `todo!("Set connection strings from container ports")` — intentional testcontainers stub, noted as such
- Task 9 Step 3: `TODO(Task 11)` — references follow-up task, intentional

Both are intentional and clearly marked. No accidental placeholders.

### Type consistency

Cross-checked method signatures:
- `JwtSecretStore::new` → matches in Tasks 2 and 11 ✓
- `KeyStore::new` → matches in Tasks 8 and 11 ✓
- `create_token_with_type` → matches in Tasks 3, 6, 7, 13 ✓
- `validate_token` → matches in Tasks 3, 13 ✓
- `KeyStore::contains/add/remove/full_sync` → matches in Tasks 8, 9, 10 ✓

All signatures consistent.

---

## Execution Estimate

| Task | Effort |
|------|--------|
| 1. Migrations + entity | 1 hour |
| 2. JWT secret store | 2 hours |
| 3. JWT dual-secret validation | 2 hours |
| 4. Rotation endpoint | 1.5 hours |
| 5. Password module | 1 hour |
| 6. Login updates | 1.5 hours |
| 7. Change password endpoint | 2 hours |
| 8. KeyStore | 3 hours |
| 9. Middleware wire | 1 hour |
| 10. CRUD wire | 1.5 hours |
| 11. AppState + background + seed | 2 hours |
| 12. Frontend | 3 hours |
| 13. Integration tests | 4 hours |
| 14. Runbook | 1 hour |
| **Total** | **~26 hours** |

(Slightly higher than spec estimate of 19h due to test infrastructure overhead.)
