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
if [ ! -d "$FRONTEND_DIST" ] || [ ! -f "$FRONTEND_DIST/index.html" ]; then
    echo ">> Building frontend (first time)..."
    (cd "$FRONTEND_SRC" && trunk build --dist "../$FRONTEND_DIST" --release 2>&1 | tail -1)
    cp -r "$FRONTEND_SRC/style" "$FRONTEND_DIST/style" 2>/dev/null || true
elif [ "$(find "$FRONTEND_SRC/src" -newer "$FRONTEND_DIST/index.html" -type f 2>/dev/null | wc -l)" -gt 0 ]; then
    echo ">> Rebuilding frontend (source changed)..."
    (cd "$FRONTEND_SRC" && trunk build --dist "../$FRONTEND_DIST" --release 2>&1 | tail -1)
    cp -r "$FRONTEND_SRC/style" "$FRONTEND_DIST/style" 2>/dev/null || true
else
    echo ">> Frontend up-to-date"
    cp -r "$FRONTEND_SRC/style" "$FRONTEND_DIST/style" 2>/dev/null || true
fi

# 3. Start server
echo ">> Starting server on 0.0.0.0:3000"
echo ">> Config: $CONFIG"
echo ">> Dashboard: http://localhost:3000"
echo ""
exec ./target/release/airouter
