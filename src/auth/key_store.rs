//! Distributed API key hash store.
//!
//! Source of truth: Redis SET `auth:key_hashes`.
//! Per-instance cache: `ArcSwap<HashSet<String>>` for fast lookups.
//! Sync: pub/sub channel `auth:key_invalidate` for near-instant invalidation
//! across instances, 5-min periodic full sync as backstop for missed messages.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use futures::StreamExt;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

/// Default Redis SET key for storing API key hashes.
pub const DEFAULT_SET_KEY: &str = "auth:key_hashes";

/// Default Redis pub/sub channel for invalidation messages.
pub const DEFAULT_CHANNEL: &str = "auth:key_invalidate";

/// Distributed cache of API key hashes backed by a Redis SET.
///
/// Combines an in-process `ArcSwap<HashSet<String>>` for fast lock-free reads
/// with Redis SET operations for distributed coordination.
///
/// # Background tasks
///
/// - [`spawn_invalidation_listener`]: subscribes to a Redis pub/sub channel
///   and removes hashes from the local in-process cache when invalidation
///   messages are received.
/// - [`spawn_periodic_sync`]: full re-sync from the Redis SET every 5 minutes
///   as a safety net against missed pub/sub messages.
///
/// # Failure mode
///
/// On Redis outage during key lookup (cache miss), the store falls back to
/// the stale in-process cache (fail-open). On CRUD operations (add/remove)
/// Redis is required (fail-closed, returns error).
pub struct KeyStore {
    /// Managed Redis connection for SET operations (SADD, SREM, SMEMBERS,
    /// SISMEMBER, PUBLISH).
    mgr: Option<ConnectionManager>,

    /// Connection info for creating a dedicated pub/sub subscription.
    client: Option<redis::Client>,

    /// In-process cache for lock-free reads on the hot path.
    cache: Arc<ArcSwap<HashSet<String>>>,

    /// Redis SET key (default: `auth:key_hashes`).
    set_key: String,

    /// Redis pub/sub channel (default: `auth:key_invalidate`).
    channel: String,
}

impl KeyStore {
    /// Create a new KeyStore and populate the in-process cache from Redis.
    ///
    /// Fails if the initial `SMEMBERS` call against Redis fails.
    ///
    /// The `mgr` is used for SET operations (fast managed connections),
    /// while `client` is an owned [`redis::Client`] used only when spawning
    /// the invalidation listener (pub/sub requires a dedicated connection).
    pub async fn new(mgr: ConnectionManager, client: redis::Client) -> anyhow::Result<Arc<Self>> {
        let set_key = std::env::var("KEY_HASH_REDIS_SET")
            .unwrap_or_else(|_| DEFAULT_SET_KEY.to_string());
        let channel = std::env::var("KEY_HASH_PUBSUB_CHANNEL")
            .unwrap_or_else(|_| DEFAULT_CHANNEL.to_string());
        let store = Arc::new(Self {
            mgr: Some(mgr),
            client: Some(client),
            cache: Arc::new(ArcSwap::from_pointee(HashSet::new())),
            set_key,
            channel,
        });
        store.full_sync().await?;
        Ok(store)
    }

    /// Check whether `hash` is present in the key set.
    ///
    /// **Fast path** (no I/O): in-process `ArcSwap` lookup.
    /// **Slow path** (only on cache miss): Redis `SISMEMBER`, then update
    /// the in-process cache on hit.
    /// **On Redis error**: fall back to stale in-process cache (fail-open).
    pub async fn contains(&self, hash: &str) -> bool {
        // Fast path: in-process cache
        if self.cache.load().contains(hash) {
            return true;
        }

        // Slow path: query Redis
        let mut conn = self.mgr();
        match redis::cmd("SISMEMBER")
            .arg(&self.set_key)
            .arg(hash)
            .query_async(&mut conn)
            .await
        {
            Ok(true) => {
                self.cache_insert(hash);
                true
            }
            Ok(false) => false,
            Err(_) => {
                tracing::warn!("Redis unreachable during contains, using stale cache");
                // Fail-open: use stale in-process cache
                self.cache.load().contains(hash)
            }
        }
    }

    /// Add `hash` to the Redis SET and update the in-process cache.
    ///
    /// Also publishes an invalidation message on the pub/sub channel so
    /// other instances can invalidate their local caches.
    ///
    /// Fails if Redis is unreachable (fail-closed for mutations).
    pub async fn add(&self, hash: &str) -> anyhow::Result<()> {
        let mut conn = self.mgr();
        conn.sadd::<_, _, ()>(&self.set_key, hash).await?;
        conn.publish::<_, _, ()>(&self.channel, hash).await?;
        self.cache_insert(hash);
        tracing::debug!(hash = %hash, "KeyStore: added hash");
        Ok(())
    }

    /// Remove `hash` from the Redis SET and update the in-process cache.
    ///
    /// Also publishes an invalidation message on the pub/sub channel so
    /// other instances can invalidate their local caches.
    ///
    /// Fails if Redis is unreachable (fail-closed for mutations).
    pub async fn remove(&self, hash: &str) -> anyhow::Result<()> {
        let mut conn = self.mgr();
        conn.srem::<_, _, ()>(&self.set_key, hash).await?;
        conn.publish::<_, _, ()>(&self.channel, hash).await?;
        self.cache_remove(hash);
        tracing::debug!(hash = %hash, "KeyStore: removed hash");
        Ok(())
    }

    /// Full re-sync: reload the entire SET from Redis into the in-process
    /// cache, replacing whatever was there before.
    pub async fn full_sync(&self) -> anyhow::Result<()> {
        let mut conn = self.mgr();
        let members: Vec<String> = conn.smembers::<_, Vec<String>>(&self.set_key).await?;
        let count = members.len();
        let new_set: HashSet<String> = members.into_iter().collect();
        self.cache.store(Arc::new(new_set));
        tracing::debug!(count = %count, "KeyStore: full sync complete");
        Ok(())
    }

    /// Spawn a background task that subscribes to the invalidation pub/sub
    /// channel and removes the notified hash from the in-process cache.
    ///
    /// On connection error, retries after a 1-second delay.
    ///
    /// The listener removes the hash from the local cache (not a full sync)
    /// because the payload is the exact hash that was added or removed.
    /// An added hash will be fetched on the next `contains` call (cache
    /// miss triggers a Redis lookup).
    pub fn spawn_invalidation_listener(
        self: &Arc<Self>,
    ) -> tokio::task::JoinHandle<()> {
        let store = Arc::clone(self);
        let channel = self.channel.clone();
        let client = self.client();
        tokio::spawn(async move {
            loop {
                if let Err(e) = store.run_listener(&channel, &client).await {
                    tracing::error!(
                        error = %e,
                        "KeyStore: invalidation listener error, reconnecting in 1s"
                    );
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    }

    /// Inner loop for the invalidation listener.
    ///
    /// Creates a new pub/sub connection, subscribes, and streams messages
    /// until the connection drops.
    async fn run_listener(
        &self,
        channel: &str,
        client: &redis::Client,
    ) -> anyhow::Result<()> {
        let mut pubsub = client.get_async_pubsub().await?;
        pubsub.subscribe(channel).await?;
        tracing::info!(channel = %channel, "KeyStore: subscribed to invalidation channel");
        let mut stream = pubsub.on_message();
        while let Some(msg) = stream.next().await {
            let payload: String = msg.get_payload()?;
            self.cache_remove(&payload);
        }
        tracing::warn!("KeyStore: invalidation listener stream ended");
        Ok(())
    }

    /// Spawn a background task that re-syncs the entire in-process cache
    /// from the Redis SET every 5 minutes.
    ///
    /// This is a safety net against missed pub/sub messages or a listener
    /// that has been disconnected for longer than the reconnect delay.
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

    /// Number of entries in the in-process cache.
    pub fn len(&self) -> usize {
        self.cache.load().len()
    }

    /// Whether the in-process cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.load().is_empty()
    }

    // ── Redis accessors ──────────────────────────────────────────────

    /// Return a cloned `ConnectionManager`, panicking if the store was
    /// constructed without one (test-only KeyStore).
    #[inline]
    fn mgr(&self) -> ConnectionManager {
        self.mgr.clone().expect("KeyStore: Redis connection manager not configured")
    }

    /// Return a cloned `redis::Client`, panicking if the store was
    /// constructed without one (test-only KeyStore).
    #[inline]
    fn client(&self) -> redis::Client {
        self.client.clone().expect("KeyStore: Redis client not configured")
    }

    // ── Private helpers ──────────────────────────────────────────────

    /// Insert a hash into the in-process cache using read-copy-update.
    fn cache_insert(&self, hash: &str) {
        let owned = hash.to_string();
        self.cache.rcu(|set| {
            let mut new_set = (**set).clone();
            new_set.insert(owned.clone());
            Arc::new(new_set)
        });
    }

    /// Remove a hash from the in-process cache using read-copy-update.
    fn cache_remove(&self, hash: &str) {
        self.cache.rcu(|set| {
            let mut new_set = (**set).clone();
            new_set.remove(hash);
            Arc::new(new_set)
        });
    }

    /// Check whether a hash is in the in-process cache without contacting Redis.
    fn cache_contains(&self, hash: &str) -> bool {
        self.cache.load().contains(hash)
    }
}

#[cfg(test)]
impl KeyStore {
    /// Create a KeyStore with pre-populated cache and no Redis connection.
    /// Only cache methods (cache_insert, cache_remove, cache_contains) and
    /// the cache-hit branch of `contains()` are safe to call.
    pub fn new_with_cache(cache: HashSet<String>) -> Self {
        Self {
            mgr: None,
            client: None,
            cache: Arc::new(ArcSwap::from_pointee(cache)),
            set_key: DEFAULT_SET_KEY.to_string(),
            channel: DEFAULT_CHANNEL.to_string(),
        }
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

    #[test]
    fn cache_insert_adds_hash_to_in_process_cache() {
        let store = KeyStore::new_with_cache(HashSet::new());
        store.cache_insert("abc123");
        assert!(store.cache_contains("abc123"));
    }

    #[test]
    fn cache_remove_removes_hash_from_in_process_cache() {
        let mut hashes = HashSet::new();
        hashes.insert("abc123".to_string());
        let store = KeyStore::new_with_cache(hashes);
        store.cache_remove("abc123");
        assert!(!store.cache_contains("abc123"));
    }

    #[test]
    fn cache_contains_checks_in_process_cache_only() {
        let store = KeyStore::new_with_cache(HashSet::new());
        assert!(!store.cache_contains("anything"));
    }

    #[test]
    fn contains_returns_true_for_cached_hash_without_redis() {
        let mut hashes = HashSet::new();
        hashes.insert("cached-hash".to_string());
        let store = KeyStore::new_with_cache(hashes);
        // Cache hit short-circuits before touching Redis.
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(rt.block_on(store.contains("cached-hash")));
    }

    #[test]
    fn len_and_is_empty_reflect_cache_state() {
        let hashes: HashSet<String> = ["a".to_string()].into();
        let store = KeyStore::new_with_cache(hashes);
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        let empty = KeyStore::new_with_cache(HashSet::new());
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }
}
