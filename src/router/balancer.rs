use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use dashmap::DashMap;
use std::time::Instant;

/// Round-robin load balancer for providers within a provider group
pub struct LoadBalancer {
    counter: AtomicUsize,
    cooldowns: Arc<DashMap<String, Instant>>,
    cooldown_duration_secs: u64,
}

impl LoadBalancer {
    pub fn new(cooldown_secs: u64) -> Self {
        Self {
            counter: AtomicUsize::new(0),
            cooldowns: Arc::new(DashMap::new()),
            cooldown_duration_secs: cooldown_secs,
        }
    }

    pub fn next_index(&self, len: usize) -> usize {
        if len == 0 { return 0; }
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % len;
        idx
    }

    pub fn mark_cooldown(&self, provider_name: &str) {
        self.cooldowns.insert(provider_name.to_string(), Instant::now());
    }

    pub fn is_on_cooldown(&self, provider_name: &str) -> bool {
        if let Some(entry) = self.cooldowns.get(provider_name) {
            if entry.elapsed().as_secs() < self.cooldown_duration_secs {
                return true;
            }
            // Expired, remove it
            drop(entry);
            self.cooldowns.remove(provider_name);
        }
        false
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

        // All on cooldown — return the first one anyway
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

    /// Clear cooldown for a provider (called on success)
    pub fn clear_cooldown(&self, provider_name: &str) {
        self.cooldowns.remove(provider_name);
    }
}
