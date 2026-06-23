# ADR 001: JWT Dual-Secret Rotation

**Date:** 2026-06-23
**Status:** Accepted

## Context

JWT secret was generated randomly at each server restart, invalidating all
existing sessions. For multi-instance deployment, all instances must share
the same secret.

## Decision

Store secrets in a Postgres singleton table (`jwt_secrets`, id=1) with
dual-secret pattern: `current_secret` (signing) + `previous_secret`
(validation-only during grace period). Rotation moves current to previous
with configurable TTL (default 24h).

## Consequences

- Sessions survive restart (secret persisted)
- Zero-downtime rotation via grace period
- Background poll (5 min) propagates rotation to all instances
- Trade-off: Postgres becomes auth critical path at startup
