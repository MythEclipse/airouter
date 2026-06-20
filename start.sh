#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

FRONTEND_DIST="frontend-dist"
FRONTEND_SRC="frontend"
CONFIG="${AIROUTER_CONFIG:-config.yaml}"

echo "=== AIRouter: Build & Start ==="

# 1. Build backend
echo ">> Building backend..."
cargo build --release 2>&1 | tail -1

# 2. Build frontend if source changed or dist missing
if [ ! -d "$FRONTEND_DIST" ] || [ "$FRONTEND_SRC/src" -nt "$FRONTEND_DIST/index.html" ] 2>/dev/null || [ ! -f "$FRONTEND_DIST/index.html" ]; then
    echo ">> Building frontend..."
    (cd "$FRONTEND_SRC" && trunk build --dist "../$FRONTEND_DIST" --release 2>&1 | tail -1)
else
    echo ">> Frontend up-to-date"
fi

# 3. Start server
echo ">> Starting server on 0.0.0.0:3000"
echo ">> Config: $CONFIG"
echo ">> Dashboard: http://localhost:3000"
echo ""
exec ./target/release/airouter
