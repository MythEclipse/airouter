# ADR 002: Distributed Key Hash Store

**Date:** 2026-06-23
**Status:** Accepted

## Context

API key hashes were stored in an in-memory `ArcSwap<HashSet<String>>`,
which does not scale to multi-instance. Each instance had its own copy
that could diverge.

## Decision

Use Redis SET as source of truth (`auth:key_hashes`) with per-instance
in-process cache (`ArcSwap<HashSet>`). Cross-instance sync via pub/sub
channel (`auth:key_invalidate`) + 5-min periodic full sync.

Fail-open on Redis lookup failures (use stale cache). Fail-closed on
CRUD mutations (return 503).

## Consequences

- Multi-instance support with eventual consistency (<5-min window)
- Fast path stay in-process memory (no Redis hop for cached keys)
- No unsafe code, pure safe Rust
- Redis pub/sub is at-most-once — periodic sync is the backstop
