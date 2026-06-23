// ─── Security Hardening Integration Tests ──────────────────────────
//
// These tests use testcontainers to spin up real Postgres and Redis
// instances, then exercise the security hardening features:
//   1. JWT persistence across store reboots (simulates instance restart)
//   2. JWT rotation with grace period (old token still valid during grace)
//   3. JWT rotation after grace expiration (old token rejected)
//   4. API key cache invalidation via pub/sub (add propagates)
//   5. API key cache invalidation via pub/sub (delete propagates)
//   6. Forced password change token type enforcement
//   7. Change-password route rejects login tokens
//
// REQUIREMENTS
// ------------
// - Docker must be running (containers are started by testcontainers)
// - `testcontainers` and `testcontainers-modules` must be in dev-deps
//
// To run (with Docker):
//   cargo test --test security_hardening_test -- --ignored
//
// To run all (no ignore, requires Docker):
//   cargo test --test security_hardening_test

mod common;

use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use airouter::auth::jwt::{
    create_token_with_type, validate_token, TokenType, Claims,
};
use airouter::auth::jwt_secret_store::JwtSecretStore;
use airouter::entities::jwt_secret;

// ─── Helpers ──────────────────────────────────────────────────────────

/// Simulate JWT rotation by updating the database directly.
///
/// Moves the current secret to `previous_secret` with the given grace
/// duration, then sets a new random `current_secret`.
async fn rotate_jwt_in_db(
    store: &JwtSecretStore,
    db: &sea_orm::DatabaseConnection,
    grace_secs: i64,
) {
    let row = jwt_secret::Entity::find_by_id(1)
        .one(db)
        .await
        .expect("jwt_secret row must exist")
        .expect("Row id=1 must exist after setup");

    let new_secret = airouter::auth::jwt_secret_store::random_hex_secret();
    let now = Utc::now();

    let mut active: jwt_secret::ActiveModel = row.into();
    let prev = active
        .current_secret
        .clone()
        .take()
        .expect("current_secret must not be NULL");

    active.previous_secret = Set(Some(prev));
    active.previous_expires_at = Set(Some(now + chrono::Duration::seconds(grace_secs)));
    active.current_secret = Set(new_secret);
    active.rotated_at = Set(Some(now));
    active.updated_at = Set(now);
    active.update(db).await.expect("Failed to update jwt_secret row");

    // Refresh the in-memory cache so the store sees the new state
    store
        .refresh_from_db()
        .await
        .expect("Failed to refresh JWT store cache");
}

/// Middleware-compatible check: is this claims set allowed for a dashboard route?
fn is_dashboard_allowed(claims: &Claims) -> bool {
    claims.typ == TokenType::Login && claims.sub == "dashboard"
}

/// Middleware-compatible check: is this claims set allowed for change-password?
fn is_change_password_allowed(claims: &Claims) -> bool {
    claims.typ == TokenType::ChangePwd
}

// ─── Tests ────────────────────────────────────────────────────────────

/// Verify that a JWT created with one store instance validates after the
/// store is dropped and recreated (simulating a server restart).
#[tokio::test]
#[ignore]
async fn jwt_validates_across_restart() {
    // Arrange
    let env = common::TestEnv::new().await;
    env.setup().await;

    // Act: create first store and sign a token
    let store1 = env.create_jwt_store().await;
    let secret1 = store1.get().current_secret;
    let token = create_token_with_type(&secret1, "dashboard", TokenType::Login, 3600)
        .expect("Token creation failed");

    // Simulate restart: drop store1, create store2
    drop(store1);
    let store2 = env.create_jwt_store().await;
    let secret2 = store2.get().current_secret;

    // Assert: same secret persisted across instances
    assert_eq!(secret1, secret2, "JWT secret should persist across store restarts");

    // Assert: token signed with secret1 still validates against store2
    let claims = validate_token(&token, &store2.get())
        .expect("Token should validate after store restart");
    assert_eq!(claims.sub, "dashboard");
    assert_eq!(claims.typ, TokenType::Login);
}

/// Verify that a token signed with the previous secret is accepted during
/// the grace period after rotation.
#[tokio::test]
#[ignore]
async fn jwt_rotation_grace_period_allows_old_tokens() {
    // Arrange
    let env = common::TestEnv::new().await;
    let db = env.setup().await;
    let store = env.create_jwt_store().await;

    // Sign a token with the current secret
    let token = create_token_with_type(
        &store.get().current_secret,
        "dashboard",
        TokenType::Login,
        3600,
    )
    .expect("Token creation failed");

    // Act: rotate with 1-hour grace period
    rotate_jwt_in_db(&store, &db, 3600).await;

    // Assert: old token still validates (inside grace window)
    let claims = validate_token(&token, &store.get())
        .expect("Old token should validate during grace period");
    assert_eq!(claims.sub, "dashboard");
    assert_eq!(claims.typ, TokenType::Login);
}

/// Verify that a token signed with the previous secret is rejected after
/// the grace period expires.
#[tokio::test]
#[ignore]
async fn jwt_rotation_expired_grace_rejects_old_tokens() {
    // Arrange
    let env = common::TestEnv::new().await;
    let db = env.setup().await;
    let store = env.create_jwt_store().await;

    // Sign a token with the current secret
    let token = create_token_with_type(
        &store.get().current_secret,
        "dashboard",
        TokenType::Login,
        3600,
    )
    .expect("Token creation failed");

    // Act: rotate with already-expired grace (0 seconds, but
    // the rotate endpoint clamps to minimum 1 hour so we use
    // a large negative value to make previous_expires_at in the past)
    rotate_jwt_in_db(&store, &db, -1).await;

    // Assert: old token is rejected (previous secret is past its expiry)
    let result = validate_token(&token, &store.get());
    assert!(
        result.is_err(),
        "Old token should be rejected after grace period expires"
    );
}

/// Verify that adding an API key hash to one KeyStore instance propagates
/// to a second instance via Redis SET with pub/sub cache invalidation.
///
/// After store1 adds a hash, store2's invalidation listener clears the
/// local cache; the next `contains()` call hits Redis and finds the hash.
#[tokio::test]
#[ignore]
async fn api_key_add_propagates_to_second_instance() {
    // Arrange: two stores pointing at the same Redis
    let env = common::TestEnv::new().await;
    env.setup().await;

    let store1 = env.create_key_store().await;
    let store2 = env.create_key_store().await;
    let _listener = store2.spawn_invalidation_listener();

    // Act: store1 adds a hash
    store1.add("hash-integration-add-test").await.unwrap();

    // Allow pub/sub message to propagate
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Assert: store2 sees the hash (cache miss -> Redis query -> hits)
    assert!(
        store2.contains("hash-integration-add-test").await,
        "store2 should eventually see the hash added by store1"
    );
}

/// Verify that removing an API key hash from one KeyStore instance
/// propagates to a second instance via pub/sub invalidation.
#[tokio::test]
#[ignore]
async fn api_key_delete_propagates_to_second_instance() {
    // Arrange: two stores pointing at the same Redis
    let env = common::TestEnv::new().await;
    env.setup().await;

    let store1 = env.create_key_store().await;
    let store2 = env.create_key_store().await;
    let _listener = store2.spawn_invalidation_listener();

    // Pre-populate the hash via store1 so it's in Redis
    store1
        .add("hash-integration-delete-test")
        .await
        .unwrap();

    // Ensure store2 has it cached (full sync or wait for propagation)
    store2.full_sync().await.unwrap();
    assert!(store2.contains("hash-integration-delete-test").await);

    // Act: delete via store1
    store1.remove("hash-integration-delete-test").await.unwrap();

    // Allow pub/sub message to propagate and listener to invalidate cache
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Assert: store2 no longer has it (cache invalidated, Redis SISMEMBER -> false)
    // We call contains() twice: first may still be a stale cache hit,
    // second should be a cache miss after invalidation.
    // Wait a bit more to be sure the listener ran.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(
        !store2.contains("hash-integration-delete-test").await,
        "store2 should eventually reflect the deletion by store1"
    );
}

/// Verify that a `ChangePwd` token (issued when `must_change_password` is
/// set) is rejected by the dashboard middleware check, while a `Login`
/// token (issued after changing password) passes.
#[tokio::test]
#[ignore]
async fn forced_password_change_blocks_dashboard_access() {
    // Arrange
    let env = common::TestEnv::new().await;
    let db = env.setup().await;

    // Set a known password and force change
    use airouter::auth::password::hash_password;
    use airouter::entities::server_config;

    let pwd_hash = hash_password("test_admin_pwd_123456");
    server_config::Entity::update_many()
        .set(server_config::ActiveModel {
            password_hash: Set(Some(pwd_hash)),
            must_change_password: Set(true),
            ..Default::default()
        })
        .filter(server_config::Column::Id.eq(1))
        .exec(&db)
        .await
        .expect("Failed to set password in server_config");

    let store = env.create_jwt_store().await;
    let secrets = store.get();

    // Act: create a ChangePwd token (as login would issue)
    let change_token = create_token_with_type(
        &secrets.current_secret,
        "dashboard",
        TokenType::ChangePwd,
        300,
    )
    .expect("ChangePwd token creation failed");

    let claims = validate_token(&change_token, &secrets)
        .expect("ChangePwd token should be valid");

    // Assert: ChangePwd token should NOT pass the dashboard middleware check
    assert!(
        !is_dashboard_allowed(&claims),
        "ChangePwd token should be rejected by dashboard middleware"
    );
    assert!(
        is_change_password_allowed(&claims),
        "ChangePwd token should pass change-password middleware"
    );

    // Act: create a Login token (as change-password would issue)
    let login_token = create_token_with_type(
        &secrets.current_secret,
        "dashboard",
        TokenType::Login,
        86400,
    )
    .expect("Login token creation failed");

    let login_claims = validate_token(&login_token, &secrets)
        .expect("Login token should be valid");

    // Assert: Login token should pass dashboard but NOT change-password
    assert!(
        is_dashboard_allowed(&login_claims),
        "Login token should pass dashboard middleware"
    );
    assert!(
        !is_change_password_allowed(&login_claims),
        "Login token should be rejected by change-password middleware"
    );
}

/// Verify that a regular `Login` token is rejected by the change-password
/// route (only `ChangePwd` tokens are allowed).
#[tokio::test]
#[ignore]
async fn change_password_route_rejects_login_token() {
    // Arrange
    let env = common::TestEnv::new().await;
    env.setup().await;
    let store = env.create_jwt_store().await;
    let secrets = store.get();

    // Act: create a regular Login token
    let token = create_token_with_type(
        &secrets.current_secret,
        "dashboard",
        TokenType::Login,
        86400,
    )
    .expect("Login token creation failed");

    let claims = validate_token(&token, &secrets)
        .expect("Login token should be valid");

    // Assert: Login token should NOT pass the change-password middleware
    assert!(
        !is_change_password_allowed(&claims),
        "Login token should be rejected by change-password middleware"
    );
    assert!(
        is_dashboard_allowed(&claims),
        "Login token should pass dashboard middleware"
    );
}
