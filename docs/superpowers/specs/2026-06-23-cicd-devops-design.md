# CI/CD & DevOps Hardening Design

**Date:** 2026-06-23
**Scope:** Four independent DevOps improvements

## Summary

Production-ready CI/CD pipeline and developer tooling for the AIRouter
project. Four independent items:

1. **GitHub Actions CI** — run tests, lint, build frontend, e2e on push/PR
2. **Dependabot** — automated Cargo dependency updates, weekly
3. **Docker healthcheck** — readiness probe for docker-compose
4. **Pre-commit hooks** — rustfmt & clippy via `lefthook` or `cargo-husky`

## Decomposition

| Item | Files | Effort | Dependencies |
|------|-------|--------|--------------|
| Docker healthcheck | `docker-compose.yml` | 0.5h | Health endpoint (done in Observability) |
| Pre-commit hooks | `.lefthook.yaml`, `.clippy.toml` | 1h | None |
| Dependabot | `.github/dependabot.yml` | 0.5h | None |
| GitHub Actions | `.github/workflows/ci.yml` | 2h | All of the above |

Order: healthcheck → pre-commit → dependabot → CI (CI depends on all).

## 1. Docker Healthcheck

Add to `docker-compose.yml`:
```yaml
services:
  airouter:
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health/ready"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 15s
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_started
```

Postgres healthcheck:
```yaml
postgres:
  healthcheck:
    test: ["CMD-SHELL", "pg_isready -U postgres"]
    interval: 10s
    timeout: 5s
    retries: 5
```

## 2. Pre-commit Hooks (lefthook)

Use `lefthook` (Go-based, fast, cross-platform):

```yaml
# .lefthook.yaml
pre-commit:
  parallel: true
  commands:
    cargo-check:
      run: cargo check --workspace --all-targets
      stage_fixed: true
    rustfmt:
      run: cargo fmt --all --check
    clippy:
      run: cargo clippy --workspace -- -D warnings
```

Install: `lefthook install` (run once per clone, or auto in CI).

**Constraint:** Only check staged files if possible. Otherwise full check is OK for small project.

## 3. Dependabot

```yaml
# .github/dependabot.yml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
      day: "monday"
    open-pull-requests-limit: 10
    labels:
      - "dependencies"
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
    open-pull-requests-limit: 5
```

## 4. GitHub Actions CI

```yaml
# .github/workflows/ci.yml
name: CI
on:
  push:
    branches: [main, "feature/*", "fix/*"]
  pull_request:
    branches: [main]

jobs:
  quality:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports: ["5432:5432"]
      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports: ["6379:6379"]

    steps:
      - uses: actions/checkout@v4
      - uses: actions-rust-lang/setup-rust-toolchain@v1
        with:
          components: clippy, rustfmt

      - name: Rust Cache
        uses: Swatinem/rust-cache@v2

      - name: Format check
        run: cargo fmt --all --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Unit tests
        run: cargo test --lib
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:5432/postgres
          REDIS_URL: redis://localhost:6379

      - name: Integration tests
        run: cargo test
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:5432/postgres
          REDIS_URL: redis://localhost:6379

  frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Trunk build
        working-directory: frontend
        run: |
          curl -fsSL https://trunk.io/releases/trunk-x86_64-unknown-linux-gnu.tar.gz | tar xz
          ./trunk build --dist ../frontend-dist
```

## Files to Create

| File | Purpose |
|------|---------|
| `.github/workflows/ci.yml` | CI pipeline |
| `.github/dependabot.yml` | Auto-dependency updates |
| `.lefthook.yaml` | Pre-commit hooks config |
| (modify) `docker-compose.yml` | Healthchecks |

## Implementation Estimate

| Item | Effort |
|------|--------|
| Docker healthcheck | 0.5h |
| Pre-commit hooks | 1h |
| Dependabot | 0.5h |
| GitHub Actions CI | 2h |
| **Total** | **~4h** |
