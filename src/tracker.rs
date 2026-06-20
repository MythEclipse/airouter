use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderMetrics {
    pub name: String,
    pub request_count: u64,
    pub error_count: u64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalMetrics {
    pub total_requests: u64,
    pub total_errors: u64,
    pub avg_latency_ms: f64,
}

#[derive(Clone)]
pub struct RequestTracker {
    total_requests: Arc<AtomicU64>,
    total_errors: Arc<AtomicU64>,
    total_latency: Arc<AtomicU64>, // sum in ms
    per_provider: Arc<DashMap<String, ProviderStats>>,
}

#[derive(Debug, Clone, Default)]
struct ProviderStats {
    request_count: u64,
    error_count: u64,
    total_latency: u64,
}

impl RequestTracker {
    pub fn new() -> Self {
        Self {
            total_requests: Arc::new(AtomicU64::new(0)),
            total_errors: Arc::new(AtomicU64::new(0)),
            total_latency: Arc::new(AtomicU64::new(0)),
            per_provider: Arc::new(DashMap::new()),
        }
    }

    pub fn record_request(&self, provider_name: &str, _model: &str, latency_ms: u64, success: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency.fetch_add(latency_ms, Ordering::Relaxed);

        if !success {
            self.total_errors.fetch_add(1, Ordering::Relaxed);
        }

        self.per_provider
            .entry(provider_name.to_string())
            .and_modify(|stats| {
                stats.request_count += 1;
                stats.total_latency += latency_ms;
                if !success {
                    stats.error_count += 1;
                }
            })
            .or_insert_with(|| {
                ProviderStats {
                    request_count: 1,
                    error_count: if !success { 1 } else { 0 },
                    total_latency: latency_ms,
                }
            });
    }

    pub fn global_metrics(&self) -> GlobalMetrics {
        let total = self.total_requests.load(Ordering::Relaxed);
        let errors = self.total_errors.load(Ordering::Relaxed);
        let latency = self.total_latency.load(Ordering::Relaxed);
        GlobalMetrics {
            total_requests: total,
            total_errors: errors,
            avg_latency_ms: if total > 0 { latency as f64 / total as f64 } else { 0.0 },
        }
    }

    pub fn provider_metrics(&self) -> Vec<ProviderMetrics> {
        let mut result: Vec<ProviderMetrics> = self.per_provider
            .iter()
            .map(|entry| {
                let stats = entry.value();
                ProviderMetrics {
                    name: entry.key().clone(),
                    request_count: stats.request_count,
                    error_count: stats.error_count,
                    avg_latency_ms: if stats.request_count > 0 {
                        stats.total_latency as f64 / stats.request_count as f64
                    } else {
                        0.0
                    },
                }
            })
            .collect();
        result.sort_by(|a, b| b.request_count.cmp(&a.request_count));
        result
    }
}
