use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use crate::provider::ErrorClass;

/// Redis-backed load balancer with cooldowns
pub struct LoadBalancer {
    counter: AtomicUsize,
    redis: redis::aio::ConnectionManager,
    base_cooldown_secs: u64,
}

impl LoadBalancer {
    pub fn new(redis: redis::aio::ConnectionManager, cooldown_secs: u64) -> Self {
        Self {
            counter: AtomicUsize::new(0),
            redis,
            base_cooldown_secs: cooldown_secs,
        }
    }

    pub fn next_index(&self, len: usize) -> usize {
        if len == 0 { return 0; }
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % len;
        idx
    }

    pub async fn mark_cooldown(&self, provider_name: &str) {
        let key = format!("cooldown:{}", provider_name);
        let mut conn = self.redis.clone();
        let _: Result<(), _> = redis::cmd("SET")
            .arg(&key).arg("1")
            .arg("EX").arg(self.base_cooldown_secs)
            .query_async(&mut conn).await;
    }

    pub async fn mark_cooldown_with_class(&self, provider_name: &str, class: ErrorClass) {
        let base = self.base_cooldown_secs;
        let duration = match class {
            ErrorClass::RateLimited => base * 2,
            ErrorClass::Transient => base,
            ErrorClass::BadRequest | ErrorClass::ServerError => 0,
        };
        if duration > 0 {
            let key = format!("cooldown:{}", provider_name);
            let mut conn = self.redis.clone();
            let _: Result<(), _> = redis::cmd("SET")
                .arg(&key).arg("1")
                .arg("EX").arg(duration)
                .query_async(&mut conn).await;
        }
    }

    pub async fn is_on_cooldown(&self, provider_name: &str) -> bool {
        let key = format!("cooldown:{}", provider_name);
        let mut conn = self.redis.clone();
        let result: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn).await.unwrap_or(None);
        result.is_some()
    }

    pub async fn clear_cooldown(&self, provider_name: &str) {
        let key = format!("cooldown:{}", provider_name);
        let mut conn = self.redis.clone();
        let _: Result<(), _> = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn).await;
    }

    pub async fn select_by_name(&self, names: &[String]) -> Option<String> {
        if names.is_empty() { return None; }
        let len = names.len();
        let start = self.next_index(len);

        for i in 0..len {
            let idx = (start + i) % len;
            if !self.is_on_cooldown(&names[idx]).await {
                return Some(names[idx].clone());
            }
        }

        Some(names[start].clone())
    }
}
