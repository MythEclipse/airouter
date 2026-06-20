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

/// Redis-backed metrics store
#[derive(Clone)]
pub struct RequestTracker;

impl RequestTracker {
    pub fn new() -> Self {
        Self
    }

    pub async fn record_request(
        &self,
        redis: &redis::aio::ConnectionManager,
        provider_name: &str,
        _model: &str,
        latency_ms: u64,
        success: bool,
    ) {
        {
            let mut conn = redis.clone();
            let _: Result<(), _> = redis::cmd("INCR")
                .arg("metrics:total_requests")
                .query_async(&mut conn).await;
        }
        {
            let mut conn = redis.clone();
            let _: Result<(), _> = redis::cmd("INCRBY")
                .arg("metrics:total_latency").arg(latency_ms as i64)
                .query_async(&mut conn).await;
        }
        {
            let mut conn = redis.clone();
            let pkey = format!("metrics:provider:{}", provider_name);
            let _: Result<(), _> = redis::cmd("HINCRBY")
                .arg(&pkey).arg("request_count").arg(1)
                .query_async(&mut conn).await;
        }
        {
            let mut conn = redis.clone();
            let pkey = format!("metrics:provider:{}", provider_name);
            let _: Result<(), _> = redis::cmd("HINCRBY")
                .arg(&pkey).arg("total_latency").arg(latency_ms as i64)
                .query_async(&mut conn).await;
        }
        if !success {
            let mut conn = redis.clone();
            let _: Result<(), _> = redis::cmd("INCR")
                .arg("metrics:total_errors")
                .query_async(&mut conn).await;
            let mut conn = redis.clone();
            let pkey = format!("metrics:provider:{}", provider_name);
            let _: Result<(), _> = redis::cmd("HINCRBY")
                .arg(&pkey).arg("error_count").arg(1)
                .query_async(&mut conn).await;
        }
    }

    pub async fn global_metrics(&self, redis: &redis::aio::ConnectionManager) -> GlobalMetrics {
        let total: u64 = {
            let mut conn = redis.clone();
            redis::cmd("GET").arg("metrics:total_requests")
                .query_async(&mut conn).await.unwrap_or(0)
        };
        let errors: u64 = {
            let mut conn = redis.clone();
            redis::cmd("GET").arg("metrics:total_errors")
                .query_async(&mut conn).await.unwrap_or(0)
        };
        let latency: u64 = {
            let mut conn = redis.clone();
            redis::cmd("GET").arg("metrics:total_latency")
                .query_async(&mut conn).await.unwrap_or(0)
        };

        GlobalMetrics {
            total_requests: total,
            total_errors: errors,
            avg_latency_ms: if total > 0 { latency as f64 / total as f64 } else { 0.0 },
        }
    }

    pub async fn provider_metrics(
        &self,
        redis: &redis::aio::ConnectionManager,
        provider_names: &[String],
    ) -> Vec<ProviderMetrics> {
        let mut results = Vec::new();
        for name in provider_names {
            let key = format!("metrics:provider:{}", name);
            let mut conn = redis.clone();
            let stats: Option<std::collections::HashMap<String, String>> =
                redis::cmd("HGETALL").arg(&key)
                    .query_async(&mut conn).await.unwrap_or(None);

            if let Some(s) = stats {
                let reqs: u64 = s.get("request_count").and_then(|v| v.parse().ok()).unwrap_or(0);
                let errs: u64 = s.get("error_count").and_then(|v| v.parse().ok()).unwrap_or(0);
                let lat: u64 = s.get("total_latency").and_then(|v| v.parse().ok()).unwrap_or(0);
                results.push(ProviderMetrics {
                    name: name.clone(),
                    request_count: reqs,
                    error_count: errs,
                    avg_latency_ms: if reqs > 0 { lat as f64 / reqs as f64 } else { 0.0 },
                });
            }
        }
        results.sort_by(|a, b| b.request_count.cmp(&a.request_count));
        results
    }
}
