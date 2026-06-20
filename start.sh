#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

FRONTEND_DIST="frontend-dist"
FRONTEND_SRC="frontend"

echo "=== AIRouter: Build & Start ==="

# 1. Build backend
echo ">> Building backend..."
cargo build --release 2>&1 | tail -1

# 2. Build Tailwind CSS
echo ">> Building CSS..."
(cd "$FRONTEND_SRC" && npm run css 2>&1 | tail -1)

# 3. Build frontend if source changed or dist missing
if [ ! -d "$FRONTEND_DIST" ] || [ ! -f "$FRONTEND_DIST/index.html" ]; then
    echo ">> Building frontend (first time)..."
    (cd "$FRONTEND_SRC" && trunk build --dist "../$FRONTEND_DIST" --release 2>&1 | tail -1)
elif [ "$(find "$FRONTEND_SRC/src" -newer "$FRONTEND_DIST/index.html" -type f 2>/dev/null | wc -l)" -gt 0 ]; then
    echo ">> Rebuilding frontend (source changed)..."
    (cd "$FRONTEND_SRC" && trunk build --dist "../$FRONTEND_DIST" --release 2>&1 | tail -1)
else
    echo ">> Frontend up-to-date"
fi

# 4. Start server
echo ">> Starting server on 0.0.0.0:3000"
echo ">> Dashboard: http://localhost:3000"
echo ""
exec ./target/release/airouter
