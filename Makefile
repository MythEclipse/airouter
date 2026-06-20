.PHONY: all build build-backend build-frontend run dev clean help

BACKEND_BIN = target/release/airouter
FRONTEND_DIST = frontend-dist

all: build

# ─── Build ───────────────────────────────────────────────────────

build: build-backend build-frontend

build-backend:
	cargo build --release

build-frontend:
	cd frontend && trunk build --dist "../${FRONTEND_DIST}" --release
	cp -r frontend/style ${FRONTEND_DIST}/style 2>/dev/null || true

# ─── Run ─────────────────────────────────────────────────────────

run: build
	./${BACKEND_BIN}

# ─── Dev mode (hot reload backend) ───────────────────────────────

dev:
	cargo run

# ─── Dev mode (frontend hot reload via Trunk) ────────────────────

dev-frontend:
	cd frontend && trunk serve --dist "../${FRONTEND_DIST}" --port 8080 --proxy-backend http://localhost:3000

# ─── Test ────────────────────────────────────────────────────────

test:
	cargo test

test-e2e:
	@echo ">> Starting server for E2E test..."
	@./target/release/airouter &
	@sleep 3
	@cd e2e && node test.mjs; kill %1 2>/dev/null || true

test-e2e-quick:
	@cd e2e && node test.mjs

test-all: test test-e2e

test-watch:
	cargo test 2>/dev/null; cargo test 2>&1 | grep -E "^test |test result"

# ─── Clean ───────────────────────────────────────────────────────

clean:
	rm -rf ${FRONTEND_DIST}
	cargo clean

# ─── Help ────────────────────────────────────────────────────────

help:
	@echo "AIRouter — LLM Gateway"
	@echo ""
	@echo "Usage: make <target>"
	@echo ""
	@echo "Build:"
	@echo "  make build          Build backend + frontend"
	@echo "  make build-backend  Build backend only"
	@echo "  make build-frontend Build frontend only"
	@echo ""
	@echo "Run:"
	@echo "  make run            Build + start server (localhost:3000)"
	@echo "  make dev            Start server (no frontend build)"
	@echo "  make dev-frontend   Frontend hot-reload on :8080"
	@echo ""
	@echo "Test:"
	@echo "  make test           Run all tests (87 tests)"
	@echo ""
	@echo "Utils:"
	@echo "  make clean          Clean build artifacts"
	@echo "  make help           This help"
