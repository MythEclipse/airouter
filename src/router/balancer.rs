use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use dashmap::DashMap;
use crate::provider::ErrorClass;

#[derive(Debug, Clone)]
struct CooldownEntry {
    since: Instant,
    duration: Duration,
}

/// Round-robin load balancer for providers within a provider group
pub struct LoadBalancer {
    counter: AtomicUsize,
    cooldowns: Arc<DashMap<String, CooldownEntry>>,
    base_cooldown_secs: u64,
}

impl LoadBalancer {
    pub fn new(cooldown_secs: u64) -> Self {
        Self {
            counter: AtomicUsize::new(0),
            cooldowns: Arc::new(DashMap::new()),
            base_cooldown_secs: cooldown_secs,
        }
    }

    pub fn next_index(&self, len: usize) -> usize {
        if len == 0 { return 0; }
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % len;
        idx
    }

    pub fn mark_cooldown(&self, provider_name: &str) {
        let duration = Duration::from_secs(self.base_cooldown_secs);
        self.cooldowns.insert(provider_name.to_string(), CooldownEntry { since: Instant::now(), duration });
    }

    pub fn mark_cooldown_with_class(&self, provider_name: &str, class: ErrorClass) {
        let base = self.base_cooldown_secs;
        let duration = match class {
            ErrorClass::RateLimited => Duration::from_secs(base * 2),
            ErrorClass::Transient => Duration::from_secs(base),
            ErrorClass::BadRequest | ErrorClass::ServerError => Duration::ZERO,
        };
        if !duration.is_zero() {
            self.cooldowns.insert(provider_name.to_string(), CooldownEntry { since: Instant::now(), duration });
        }
    }

    pub fn is_on_cooldown(&self, provider_name: &str) -> bool {
        if let Some(entry) = self.cooldowns.get(provider_name) {
            if entry.since.elapsed() < entry.duration {
                return true;
            }
            drop(entry);
            self.cooldowns.remove(provider_name);
        }
        false
    }

    pub fn clear_cooldown(&self, provider_name: &str) {
        self.cooldowns.remove(provider_name);
    }

    /// Select a provider, skipping those on cooldown
    pub fn select<'a>(&self, providers: &'a [&'a Box<dyn crate::provider::Provider>]) -> Option<&'a Box<dyn crate::provider::Provider>> {
        if providers.is_empty() { return None; }

        let len = providers.len();
        let start = self.next_index(len);

        for i in 0..len {
            let idx = (start + i) % len;
            if !self.is_on_cooldown(providers[idx].name()) {
                return Some(providers[idx]);
            }
        }

        Some(providers[start])
    }

    /// Select a provider name, skipping those on cooldown
    pub fn select_by_name(&self, names: &[String]) -> Option<String> {
        if names.is_empty() { return None; }
        let len = names.len();
        let start = self.next_index(len);

        for i in 0..len {
            let idx = (start + i) % len;
            if !self.is_on_cooldown(&names[idx]) {
                return Some(names[idx].clone());
            }
        }

        Some(names[start].clone())
    }
}
