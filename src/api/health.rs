use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use sea_orm::ConnectionTrait;

use crate::server::app::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    checks: HealthChecks,
}

#[derive(Serialize)]
struct HealthChecks {
    database: CheckResult,
    redis: CheckResult,
}

#[derive(Serialize)]
struct CheckResult {
    status: String,
    latency_ms: u64,
}

/// Simple rate limiter: max 1 request per second.
/// Uses a global static `OnceLock<Mutex<Instant>>` so it requires no
/// extra fields on `AppState`.
fn rate_limited() -> bool {
    let now = Instant::now();
    static LAST: OnceLock<Mutex<Instant>> = OnceLock::new();
    let mut last = LAST
        .get_or_init(|| Mutex::new(now - Duration::from_secs(1)))
        .lock()
        .expect("health rate-limiter lock poisoned");
    if now.duration_since(*last).as_secs() < 1 {
        return true;
    }
    *last = now;
    false
}

/// `GET /health` — Liveness probe.
///
/// Returns 200 with JSON body containing the version string and the status
/// of database and Redis dependencies, including per-check latency in
/// milliseconds. If the rate limit (1 req/s) is exceeded, returns 429.
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if rate_limited() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(HealthResponse {
                status: "rate_limited".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                checks: HealthChecks {
                    database: CheckResult {
                        status: "skipped".to_string(),
                        latency_ms: 0,
                    },
                    redis: CheckResult {
                        status: "skipped".to_string(),
                        latency_ms: 0,
                    },
                },
            }),
        );
    }

    let version = env!("CARGO_PKG_VERSION").to_string();

    // ── Database check ─────────────────────────────────────────────────
    let start = Instant::now();
    let db_status = match state
        .db
        .execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1".to_string(),
        ))
        .await
    {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("error: {}", e),
    };
    let db_latency = start.elapsed().as_millis() as u64;

    // ── Redis check ────────────────────────────────────────────────────
    let start = Instant::now();
    let redis_status = {
        let mut conn = state.redis.clone();
        match redis::cmd("PING").query_async::<String>(&mut conn).await {
            Ok(s) if s == "PONG" => "ok".to_string(),
            Ok(_) => "unexpected response".to_string(),
            Err(e) => format!("error: {}", e),
        }
    };
    let redis_latency = start.elapsed().as_millis() as u64;

    let overall = if db_status == "ok" && redis_status == "ok" {
        "ok"
    } else {
        "degraded"
    };

    (
        StatusCode::OK,
        Json(HealthResponse {
            status: overall.to_string(),
            version,
            checks: HealthChecks {
                database: CheckResult {
                    status: db_status,
                    latency_ms: db_latency,
                },
                redis: CheckResult {
                    status: redis_status,
                    latency_ms: redis_latency,
                },
            },
        }),
    )
}

/// `GET /health/ready` — Readiness probe.
///
/// Returns 200 OK if both database and Redis respond to a quick
/// `SELECT 1` / `PING`, otherwise returns 503 Service Unavailable.
/// Intended for Kubernetes readiness probes and docker-compose
/// healthchecks.  NOT rate-limited (readiness probes may fire
/// concurrently from orchestration).
pub async fn health_ready(
    State(state): State<Arc<AppState>>,
) -> StatusCode {
    // ── Database check ─────────────────────────────────────────────────
    let db_ok = state
        .db
        .execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT 1".to_string(),
        ))
        .await
        .is_ok();

    // ── Redis check ────────────────────────────────────────────────────
    let mut conn = state.redis.clone();
    let redis_ok = redis::cmd("PING")
        .query_async::<String>(&mut conn)
        .await
        .map(|s| s == "PONG")
        .unwrap_or(false);

    if db_ok && redis_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}
