# Observability Hardening Design

**Date:** 2026-06-23
**Scope:** Four independent observability improvements

## Summary

Improve AIRouter's observability posture for production operations. Four
independent improvements:

1. **Structured JSON logging** — enrich tracing fields (request_id,
   provider_name, latency, error_class), better log levels, rate-limited
   error aggregation so repeat errors don't flood logs.
2. **Enhanced Prometheus metrics** — per-endpoint request count/duration
   histogram, per-provider latency buckets, error rate by class, active
   connections gauge.
3. **Health check endpoint** — `GET /health` with liveness (server up) +
   readiness (DB/Redis connected) probes, returns dependency statuses.
4. **Request body size limits** — middleware-enforced max payload per
   endpoint type (chat completions vs admin API), clear error messages.

## Decomposition

These are four independent improvements. They can be implemented in any
order and don't share code dependencies. Recommend order:
1. Body size limits (safety first, smallest scope)
2. Health check (ops utility, no deps)
3. Structured logging (no deps)
4. Prometheus metrics (touch existing tracker + dispatch hot path)

## 1. Structured Logging

### Changes

- **tracing-subscriber init** in `src/main.rs` — already JSON format.
  Add `tower-http` request-id propagation: ensure the span logs
  `request_id`, `method`, `path`, `status`, `latency_ms` on INFO+.
- **Error events (WARN/ERROR):** inject `error.kind`, `error.detail`,
  `provider`, `model` via `tracing::error!(error.kind = %, ...)`.
- **Rate-limited error aggregation:** custom `tracing-subscriber::Layer`
  that deduplicates identical ERROR messages within a 1-second window.
  Logs first occurrence + `repeated N times` on the next unique message.

### Dependencies

- `tracing-appender` (optional, for non-blocking file output).

## 2. Enhanced Prometheus Metrics

### Files
- `src/tracker.rs` — add new metric instruments
- `src/router/core.rs` — instrument dispatch
- `src/router/balancer.rs` — update cooldown gauge
- `src/provider/trait_def.rs` — error class label helper

### New Metrics

| Metric | Type | Labels | Source |
|--------|------|--------|--------|
| `airouter_requests_total` | Counter | `method`, `path`, `status` | API handler wrappers via Tower layer |
| `airouter_request_duration_ms` | Histogram | `method`, `path` | Timer around handler |
| `airouter_provider_requests_total` | Counter | `provider`, `model`, `status` | RouteEngine.dispatch |
| `airouter_provider_latency_ms` | Histogram | `provider`, `model` | Timer around provider call |
| `airouter_provider_errors_total` | Counter | `provider`, `error_class` | ProviderError mapping |
| `airouter_cooldown_active` | Gauge | `provider` | LoadBalancer cooldown |
| `airouter_redis_connected` | Gauge | — | Redis ping at scrape time |
| `airouter_postgres_connected` | Gauge | — | DB ping at scrape time |

### Histogram Buckets

`[5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000]` ms. LLM calls
typically 200ms–5s.

### Instrumentation Points

- **HTTP handler layer:** Tower middleware wraps each request: increment
  counter on entry, histogram on completion.
- **RouteEngine::dispatch:** per-provider counter + latency before/after
  provider call.
- **ProviderError handling:** `match error_class { RateLimited => ...,
  ServerError => ...}` increments `airouter_provider_errors_total` per class.
- **Cooldown gauge:** set to 1 on `mark_cooldown`, set to 0 after expiry
  (or on shutdown).

## 3. Health Check Endpoint

### Files
- `src/api/health.rs` — NEW handler

### Endpoints

```rust
GET /health
  Returns 200 OK.
  JSON body with dependency statuses:
  {
    "status": "ok",
    "version": "1.0.0",
    "checks": {
      "database":  { "status": "ok", "latency_ms": 2 },
      "redis":     { "status": "ok", "latency_ms": 1 },
      "providers": { "status": "degraded", "provider_count": 34,
                     "healthy": 32, "unhealthy": 2 }
    }
  }

GET /health/ready
  Returns 200 OK if DB + Redis respond to a quick SELECT 1 / PING.
  Returns 503 otherwise.
  Used for Kubernetes readiness probes / docker-compose healthcheck.
```

### Access
- **NOT** behind auth middleware.
- Rate-limited internally (max 1 request/sec) to avoid hammering DB/Redis.

### Provider Health
- Lightweight: count providers whose last dispatch returned
  `ServerError` or `Transient` within the last 5 minutes. Uses a
  simple in-memory ring buffer in `RequestTracker`.

## 4. Request Body Size Limits

### Files
- `src/server/app.rs` — add `RequestBodyLimitLayer` per route group

### Approach

Use `tower_http::limit::RequestBodyLimitLayer` applied per route group:

| Route Group | Max Body Size |
|-------------|---------------|
| `/v1/chat/completions` | 2 MB |
| `/v1/messages` | 2 MB |
| `/v1/models` | 1 KB |
| `/api/dashboard/*` | 512 KB |
| `/api/auth/*` | 2 KB |
| `/api/oauth/*` | 64 KB |

`RequestBodyLimitLayer` returns 413 Payload Too Large with a canned
response body when exceeded.

### Implementation Snippet

```rust
use tower_http::limit::RequestBodyLimitLayer;

let completions_routes = Router::new()
    .route("/v1/chat/completions", post(handler))
    .layer(RequestBodyLimitLayer::new(2_000_000));
```

## Implementation Estimate

| Item | Effort |
|------|--------|
| Request body size limits | 1.5h |
| Health check endpoint | 2h |
| Structured logging | 2h |
| Enhanced Prometheus metrics | 4h |
| **Total** | **~9.5 hours** |

## Risk & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Metrics cardinality explosion | Prometheus memory | Bounded label values (no user IDs, no free-form strings) |
| Health check DDoS | Wastes DB/Redis | Rate limit `/health` to 1 req/s |
| 413 breaks legit large prompts | User frustration | 2MB = ~500K tokens. Monitor and raise if needed. |
| Metrics instrumenting via `tracing` not `metrics` | Duplicate data | Use `metrics` crate for metrics (as existing), `tracing` for logging. Separate concerns. |
