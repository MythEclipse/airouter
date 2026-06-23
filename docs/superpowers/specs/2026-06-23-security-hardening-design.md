# Security Hardening Design

**Status:** Approved (pending implementation)
**Date:** 2026-06-23
**Scope:** Multi-instance security foundation (3 improvements)

## Summary

Improve AIRouter's security foundation so it can scale to multiple instances
without losing sessions or diverging API-key state. Three independent changes:

1. **JWT secret persistence** — move from in-memory random (lost on restart,
   invalidates all sessions) to Postgres-backed dual-secret with rotation grace.
2. **Forced password change** — replace hardcoded `123456` default with random
   initial password printed to logs + mandatory change on first login.
3. **Distributed key_hashes** — replace in-memory `arc_swap` cache with Redis
   SET + per-instance cache + pub/sub invalidation so all instances stay in sync.

## Constraints & Context

- **Deployment target:** Multi-instance capable (Kubernetes / docker-compose
  replicas / etc.). Single-instance also supported.
- **User model:** Single admin (no user table).
- **Migration:** Greenfield (no existing production users). Schema changes
  are additive and idempotent.
- **Hash algorithm:** SHA-256 (current). Argon2id upgrade deferred to a
  follow-up spec — SHA-256 acceptable for single-admin internal tool with
  32-char random initial password.

## Out of Scope (deferred to follow-up specs)

- Argon2id password hashing upgrade
- Token revocation via JWT ID (`jti`) + Redis blocklist
- Argon2id migration for existing API keys (N/A — API keys already hashed)
- KMS integration for JWT secret storage
- Multi-user support
- OIDC / OAuth2 for admin login

---

## Section 1: Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Multi-Instance AIRouter                        │
│                                                                     │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐                          │
│   │Instance 1│  │Instance 2│  │Instance 3│   (semua stateless)       │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘                          │
│        │             │             │                                 │
│        └─────────────┼─────────────┘                                 │
│                      ▼                                               │
│   ┌─────────────────────────────────────────────┐                    │
│   │  PostgreSQL (shared)                        │                    │
│   │  ├─ jwt_secrets (current + previous)        │                    │
│   │  ├─ server_config (extend: password_hash,   │                    │
│   │  │   password_changed_at, must_change)      │                    │
│   │  └─ api_keys (existing)                    │                    │
│   └─────────────────────────────────────────────┘                    │
│                      ▲                                               │
│   ┌─────────────────────────────────────────────┐                    │
│   │  Redis (shared)                             │                    │
│   │  ├─ key_hashes (SET, source of truth)       │                    │
│   │  ├─ key_hashes:invalidate (Pub/Sub channel) │                    │
│   │  └─ cooldowns / rate_limits / metrics       │                    │
│   │     (existing)                              │                    │
│   └─────────────────────────────────────────────┘                    │
└─────────────────────────────────────────────────────────────────────┘
```

**Key change:** All instances are stateless. Auth state lives in Postgres
(persistent) or Redis (ephemeral). Any instance can handle any request.

---

## Section 2: JWT Secret Persistence

### Schema (`migrations/003_jwt_secrets.sql`)

```sql
CREATE TABLE IF NOT EXISTS jwt_secrets (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- singleton row
    current_secret TEXT NOT NULL,
    previous_secret TEXT,                   -- NULL saat tidak rotasi
    previous_expires_at TIMESTAMPTZ,        -- kapan previous jadi invalid
    rotated_at TIMESTAMPTZ,                 -- kapan rotasi terakhir
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### Startup Flow

1. `Database::connect()` — fail-fast if Postgres unreachable.
2. `run_migrations()` — creates `jwt_secrets` if missing.
3. `seed_jwt_secret()` — if row empty, generate random 64-char hex, INSERT.
4. `AppState.jwt_secrets: Arc<ArcSwap<JwtSecrets>>` — in-memory cache, refreshed
   every 5 minutes via background poll.

### Token Validation

```rust
pub async fn validate_token(token: &str, state: &AppState) -> Result<Claims> {
    let secrets = state.jwt_secrets.load();  // ArcSwap, lock-free read

    // 1. Try current secret
    if let Ok(claims) = decode(token, &secrets.current_secret) {
        return Ok(claims);
    }

    // 2. Try previous if still in grace period
    if let (Some(prev), Some(expires_at)) = (&secrets.previous_secret, secrets.previous_expires_at) {
        if expires_at > Utc::now() {
            if let Ok(claims) = decode(token, prev) {
                return Ok(claims);
            }
        }
    }

    Err(JwtError::InvalidToken)
}
```

### Rotation Endpoint

```
POST /api/dashboard/rotate-jwt-secret
  Auth: dashboard JWT required
  Body: { "grace_period_hours": 24 }   (default 24, max 168)

  Logic:
    1. previous_secret        = current_secret
    2. previous_expires_at    = now + grace_period_hours
    3. rotated_at             = now
    4. current_secret         = generate_random_hex(64)
    5. UPDATE jwt_secrets
    6. Refresh ArcSwap cache (this instance only)
    7. Log "JWT secret rotated, grace period until {ts}"

  Response:
    {
      "previous_expires_at": "2026-06-24T10:00:00Z",
      "instances_refreshed": 1   // other instances refresh via 5-min poll
    }
```

### Cache Refresh Strategy

- **Background poll** every 5 minutes (per instance). Acceptable staleness for
  admin operation.
- **Trade-off:** max 5-min disruption window after rotation. Acceptable.
- **Future:** Redis pub/sub for instant refresh. Not implemented now.

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| Postgres down at startup | Fail to start (fail-fast) |
| Postgres down at runtime | Cache still validates existing tokens; rotation disabled; token expiry (24h dashboard / 30d AI) becomes hard limit |
| Rotation 2× within grace period | Previous overwritten; some tokens may invalidate earlier than first rotation's grace period would suggest. Documented. |
| `previous_expires_at` past | Validation fails for tokens issued before that rotation |
| Periodic poll fails (DB hiccup) | Keep using current cache; retry next interval |

---

## Section 3: Password Management

### Schema (extend `server_config`)

```sql
ALTER TABLE server_config
    ADD COLUMN IF NOT EXISTS password_hash TEXT,
    ADD COLUMN IF NOT EXISTS password_changed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE;
```

### Initial Password Generation

At first run (when `password_hash IS NULL`):

1. Generate random 32-char alphanumeric password.
2. Compute SHA-256 hash, store in `server_config.password_hash`.
3. Set `must_change_password = true`.
4. Log plaintext password once at WARN level (high-visibility).
5. Continue startup — admin must read logs to find password.

```rust
fn generate_password(len: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                              abcdefghijklmnopqrstuvwxyz\
                              0123456789";
    let mut rng = rand::thread_rng();
    (0..len).map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char).collect()
}
```

### Login + Forced Change Flow

```
POST /api/auth/login { password }
  │
  ▼
Verify password_hash (SHA-256 compare)
  │
  ├─ INVALID → 401 Unauthorized
  │
  └─ VALID ──┐
             │
             ▼
       must_change_password?
             │
       ┌─────┴──────┐
       │            │
      YES          NO
       │            │
       ▼            ▼
   Issue         Issue
   change_token  regular JWT
   (5 min TTL,  (24h dash /
   typ=          30d ai)
   change_pwd)       │
       │            ▼
       ▼         200 OK
   200 OK       { token, must_change: false }
   { change_token, must_change: true }
       │
       ▼
   Frontend: redirect ke /change-password
       │
       ▼
POST /api/auth/change-password
  Header: Authorization: Bearer {change_token}
  Body: { new_password, confirm_password }
       │
       ▼
   Validate token (typ must = "change_pwd")
   Validate password (≥12 chars)
   Hash new password (SHA-256)
   UPDATE server_config:
     password_hash         = new hash
     password_changed_at   = now()
     must_change_password  = false
   Issue regular JWT (dashboard)
       │
       ▼
   200 OK { token }
       │
       ▼
   Frontend: redirect ke /dashboard
```

### Token Type Claim

Add `typ` field to JWT claims:

```rust
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,   // "dashboard" | "ai"
    exp: usize,
    iat: usize,
    typ: String,   // "login" | "change_pwd"   ← NEW
}
```

Middleware enforces:
- `/api/dashboard/*` → `typ=login`, `sub=dashboard`
- `/api/ai/*` → `typ=login`, `sub in {dashboard, ai}`
- `/api/auth/change-password` → `typ=change_pwd`
- `/api/auth/login` → public

### Password Validation Rules

```rust
fn validate_password_strength(pwd: &str) -> Result<(), &'static str> {
    if pwd.len() < 12 {
        return Err("Password must be at least 12 characters");
    }
    Ok(())
}
```

NIST 800-63B guideline: length > complexity.

### Frontend Changes

| Component | Change |
|-----------|--------|
| `pages/login.rs` | Detect `must_change: true` in response → redirect to `/change-password` |
| `pages/change_password.rs` (NEW) | Form: new password, confirm. Submit to `/api/auth/change-password`. Success → redirect `/dashboard` |
| `app.rs` routes | Add `/change-password` route (outside `AuthenticatedLayout`, doesn't need existing JWT) |
| `api/mod.rs` | Add `change_password()` function |

### Recovery (Forgot Password)

Documented in runbook. Recovery via direct DB update:

```sql
UPDATE server_config SET
  password_hash         = encode(sha256('new_password'), 'hex'),
  must_change_password  = true
WHERE id = 1;
```

No env var override in MVP — keep complexity low.

---

## Section 4: Distributed key_hashes

### Data Layout di Redis

```
Key:     auth:key_hashes
Type:    SET
Members: SHA-256 hashes of all active API keys

Channel: auth:key_invalidate
Type:    Pub/Sub
Payload: SHA-256 hash string
```

### Lookup Flow

```rust
async fn validate_api_key(hash: &str, store: &KeyHashStore) -> bool {
    // 1. In-process cache (microseconds)
    if store.cache.load().contains(hash) {
        return true;
    }

    // 2. Redis (milliseconds)
    match store.redis_sismember("auth:key_hashes", hash).await {
        Ok(true) => {
            store.cache_insert(hash);
            true
        }
        Ok(false) => false,
        Err(_) => {
            // Redis unreachable: fail-open dengan stale cache
            tracing::warn!("Redis unreachable, using stale key cache");
            store.cache.load().contains(hash)
        }
    }
}
```

### CRUD Flow

```rust
async fn create_api_key(name: String, key: String, ...) -> Result<...> {
    let hash = sha2_hex(&key);

    // 1. Persist to Postgres (source of truth)
    api_key::Entity::insert(...).exec(db).await?;

    // 2. Update Redis SET
    redis.sadd("auth:key_hashes", &hash).await?;

    // 3. Broadcast invalidation (sync other instances)
    redis.publish("auth:key_invalidate", &hash).await?;

    Ok(...)
}

async fn delete_api_key(id: Uuid, ...) -> Result<...> {
    let hash = lookup_hash(id).await?;

    api_key::Entity::delete_by_id(id).exec(db).await?;

    redis.srem("auth:key_hashes", &hash).await?;
    redis.publish("auth:key_invalidate", &hash).await?;

    Ok(())
}
```

### Per-Instance Background Tasks

**Task 1: Pub/Sub Listener**
- Subscribe: `auth:key_invalidate`
- On message: remove hash from in-process cache
- Next request re-fetches from Redis

**Task 2: Periodic Full Sync**
- Interval: 5 minutes
- Logic: `SMEMBERS auth:key_hashes`, replace in-process cache atomically
- Purpose: Backstop for lost pub/sub messages

### Failure Mode Analysis

| Scenario | Behavior | User Impact |
|----------|----------|-------------|
| Redis down (lookup) | Use stale in-process cache | API keys work until key changed in DB (max staleness = 5 min) |
| Redis down (CRUD) | Return 503 Service Unavailable | Admin cannot add/delete API keys until Redis up |
| Postgres down (CRUD) | Cannot persist → 503 | Admin cannot add/delete API keys |
| Pub/Sub message lost | Next periodic sync (max 5 min) recovers | Brief inconsistency window |
| Instance restart | Cold cache → first request per key hits Redis | Slightly slower first request per key (one-time) |

---

## Section 5: Migration & Rollout

### Migration Files (urutan eksekusi)

```
migrations/
├── 001_initial.sql        ← existing (do not modify)
├── 002_oauth.sql          ← existing (do not modify)
├── 003_jwt_secrets.sql    ← NEW: jwt_secrets table
└── 004_password.sql       ← NEW: ALTER server_config
```

**003_jwt_secrets.sql:**
```sql
CREATE TABLE IF NOT EXISTS jwt_secrets (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_secret TEXT NOT NULL,
    previous_secret TEXT,
    previous_expires_at TIMESTAMPTZ,
    rotated_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

**004_password.sql:**
```sql
ALTER TABLE server_config
    ADD COLUMN IF NOT EXISTS password_hash TEXT,
    ADD COLUMN IF NOT EXISTS password_changed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE;
```

### Environment Variables (.env additions)

```bash
# Optional with defaults
JWT_ROTATION_GRACE_HOURS=24                    # default 24
KEY_HASH_PUBSUB_CHANNEL=auth:key_invalidate    # default
KEY_HASH_SYNC_INTERVAL_SECS=300                # default 5 min
KEY_HASH_REDIS_SET=auth:key_hashes             # default

# Initial password always random + printed (no env required)
```

### Frontend Rebuild

```bash
cd frontend && trunk build --dist ../frontend-dist
```

Required because of new page, route, API function, and login redirect logic.

### Deployment Runbook

#### First-Time Deployment (greenfield)

```bash
# 1. Pull latest
git pull origin main

# 2. Build
cd frontend && trunk build --dist ../frontend-dist && cd ..
cargo build --release

# 3. Run migrations + seed (auto at startup)
./target/release/airouter

# 4. CAREFULLY: cat initial password from logs
journalctl -u airouter -n 100 | grep "INITIAL ADMIN PASSWORD" -A 5

# 5. Login ke dashboard, forced change password

# 6. Verify:
#    - Login dengan password baru works
#    - Add API key, test request ke /v1/chat/completions
#    - Kill instance, restart, login masih works (JWT secret persist)
```

#### Multi-Instance Deployment

```yaml
# docker-compose.yml addition
services:
  airouter:
    deploy:
      replicas: 3
    environment:
      - DATABASE_URL=postgres://...
      - REDIS_URL=redis://...
    depends_on:
      - postgres
      - redis
```

**Critical:** All instances share Postgres + Redis. JWT secret di-Postgres → otomatis shared. Key_hashes di-Redis → otomatis shared.

**Load balancer sticky session:** Not required (all instances stateless).

#### Upgrade dari Versi Lama

```bash
# 1. Stop instance
docker-compose stop airouter

# 2. Backup DB
pg_dump $DATABASE_URL > backup_pre_security_hardening.sql

# 3. Pull + build
git pull && cargo build --release && cd frontend && trunk build --dist ../frontend-dist && cd ..

# 4. Run (migrations auto at startup)
./target/release/airouter

# 5. For upgrade: initial password TIDAK di-generate (password_hash sudah ada)

# 6. Verify:
#    - Login masih works dengan password lama
#    - JWT secret rotation works
#    - API key CRUD sync ke semua instances (jika multi)
```

### Rollback Strategy

| Component | Rollback |
|-----------|----------|
| Database migration | `003` and `004` additive only. Rollback = `DROP TABLE jwt_secrets` + `ALTER TABLE server_config DROP COLUMN ...` |
| Code | Git revert. Manual schema rollback if needed for full revert |
| Old `arc_swap` key_hashes | Restore from git. Postgres & Redis unaffected |
| Old JWT secret (in-memory random) | Restore from git. All sessions invalidate (acceptable for rollback) |

**Safety:** Migrations 003 & 004 are additive only — no existing rows modified.

---

## Section 6: Testing & Verification

### Unit Tests

**`src/auth/jwt.rs`** — add tests:
- `validate_with_current_secret_succeeds`
- `validate_with_previous_secret_during_grace_period_succeeds`
- `validate_with_previous_secret_after_grace_period_fails`
- `validate_with_wrong_secret_fails`
- `change_pwd_token_rejected_on_dashboard_routes`
- `login_token_rejected_on_change_password_route`

**`src/auth/key_store.rs`** (new file) — tests:
- `key_cache_miss_triggers_redis_lookup` (mock Redis)
- `key_cache_hit_skips_redis`
- `redis_down_falls_back_to_stale_cache`
- `pubsub_message_invalidates_cache`

**`src/config/db.rs`** — tests using testcontainers:
- `seed_jwt_secret_creates_row_if_missing`
- `seed_jwt_secret_does_not_overwrite_existing`
- `seed_password_generates_when_missing`
- `seed_password_preserves_when_set`

### Integration Tests (`tests/security_hardening_test.rs`)

Multi-instance simulation scenarios:

- `jwt_validates_across_restart` — token survives instance restart
- `jwt_rotation_grace_period_allows_old_tokens`
- `jwt_rotation_expired_grace_rejects_old_tokens`
- `api_key_add_propagates_to_second_instance`
- `api_key_delete_propagates_to_second_instance`
- `forced_password_change_blocks_dashboard_access`
- `change_password_route_rejects_login_token`

### Testcontainers Setup

Use **`testcontainers`** crate for Postgres + Redis in integration tests:

```rust
use testcontainers::{Container, Docker, Image};

pub struct TestEnv {
    pub postgres_url: String,
    pub redis_url: String,
}

impl TestEnv {
    pub async fn new() -> Self {
        // Run Postgres + Redis containers, return connection strings
    }
}
```

### E2E Tests (`e2e/test.mjs` additions)

```javascript
test('forced password change flow', async ({ page }) => { /* ... */ });
test('JWT secret rotation via dashboard', async ({ page }) => { /* ... */ });
test('API key CRUD multi-instance', async ({ page }) => { /* ... */ });
```

### Manual Verification Runbook

After deploying to staging/production, run this checklist:

```markdown
## Staging Verification

- [ ] Initial password muncul di logs saat first run
- [ ] Login dengan initial password → forced change form
- [ ] Ganti password → access dashboard
- [ ] Logout, login dengan new password → works
- [ ] Logout, login dengan initial password → fails
- [ ] Add API key di dashboard
- [ ] Use API key di `curl /v1/chat/completions` → works
- [ ] Delete API key
- [ ] Use deleted API key → 401
- [ ] Kill server, restart → JWT secret persist (login masih works)
- [ ] Rotate JWT secret via dashboard → grace period set
- [ ] Existing session masih works dalam grace period
- [ ] Wait grace period → existing session invalid
- [ ] Multi-instance: add key di instance A, validate di instance B → works

## Production Verification

- [ ] Same as staging, plus:
- [ ] Monitor error rate di logs (should be 0 unexpected errors)
- [ ] Monitor request latency (should be unchanged atau improve)
- [ ] Verify Redis pub/sub messages flow (redis-cli MONITOR)
```

### Coverage Targets

| Area | Target |
|------|--------|
| `auth/jwt.rs` | 95%+ |
| `auth/middleware.rs` | 90%+ |
| `auth/key_store.rs` (new) | 95%+ |
| `config/db.rs` (new seed functions) | 80%+ |
| Migration SQL | Manual review |

---

## Implementation Estimate

| Phase | Files | Effort |
|-------|-------|--------|
| 1. Migrations + entity | 2 SQL + 2 entity updates | 1 hour |
| 2. JWT secret module | `auth/jwt.rs` rewrite + `auth/jwt_secret.rs` new | 3 hours |
| 3. Password management | `auth/password.rs` new + login/change-pwd endpoints + frontend page | 4 hours |
| 4. Distributed key_hashes | `auth/key_store.rs` new + CRUD integration + background tasks | 4 hours |
| 5. Tests | Unit + integration + e2e | 6 hours |
| 6. Runbook + docs | This spec → user-facing runbook | 1 hour |
| **Total** | | **~19 hours** |

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Migration breaks existing data | High | Additive-only migrations, tested in staging first |
| Redis outage during deploy | Medium | Fail-open in cache, fail-closed in CRUD (with clear error) |
| JWT secret lost (Postgres corruption) | Critical | Documented recovery: rotate secret, all sessions invalidate |
| Pub/Sub message loss | Low | 5-min periodic sync as backstop |
| Initial password lost in logs | Medium | Documented: must grep logs immediately after first run |
| Multi-instance cache divergence | Low | Pub/sub + periodic sync covers it |

## Acceptance Criteria

This spec is considered complete when:

1. All migrations run successfully on greenfield and existing data.
2. All unit + integration + e2e tests pass.
3. Manual verification runbook passes in staging.
4. Production deploy completed with zero unexpected errors in first 24h.
5. Documentation updated: runbook, troubleshooting guide, security notes.
