# ADR 003: Forced Password Change Flow

**Date:** 2026-06-23
**Status:** Accepted

## Context

Default password was hardcoded as SHA-256("123456"), reset on every
restart. No mechanism to enforce password changes.

## Decision

Generate random 32-char alphanumeric password at first startup, print to
logs once at WARN level. Add `must_change_password` boolean to
`server_config` table. Login with `must_change=true` issues a
`change_pwd` token (5-min TTL, limited to `/api/auth/change-password`).
Middleware enforces token type per route.

## Consequences

- Greenfield deployments get secure random password by default
- Forced change ensures no stale default passwords
- Recovery requires direct DB access (documented in runbook)
- Argon2id hashing deferred to follow-up (SHA-256 sufficient for
  single-admin internal tool with 32-char random password)
