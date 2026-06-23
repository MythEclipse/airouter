# ADR 004: Structured Logging and Error Dedup

**Date:** 2026-06-23
**Status:** Accepted

## Context

Logging used basic `tracing_subscriber::fmt().json()`. Error messages
from provider failures could flood logs during outages with no
deduplication. No structured fields (request_id, provider, model) on
error events.

## Decision

- Use `tracing_subscriber::registry()` with custom `DedupFormatEvent`
  layer wrapping JSON formatter
- Identical ERROR messages within 1-second window are suppressed;
  a summary line is emitted when a new unique error arrives
- TraceLayer enriched with request_id, method, path, status, latency_ms
- Provider error paths emit error_kind, error_detail, provider, model,
  latency_ms as structured fields

## Consequences

- Log volume drops dramatically during provider outages
- Structured fields enable log aggregation (Datadog, ELK) queries
- Dedup is consecutive-only (identical back-to-back errors), not
  full-window dedup
- Summary lines bypass structured pipeline (written to stderr)
