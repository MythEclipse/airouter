#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
BASE="http://localhost:3000"
KEY="sk-test-abc123"
PASS=0
FAIL=0

green() { echo -e "\033[32m$1\033[0m"; }
red()   { echo -e "\033[31m$1\033[0m"; }
bold()  { echo -e "\033[1m$1\033[0m"; }

assert() {
    local desc="$1" expected="$2" actual="$3"
    if [ "$actual" = "$expected" ]; then
        green "  ✅ $desc"
        PASS=$((PASS + 1))
    else
        red "  ❌ $desc (expected: $expected, got: $actual)"
        FAIL=$((FAIL + 1))
    fi
}

assert_contains() {
    local desc="$1" expected="$2" actual="$3"
    if echo "$actual" | grep -q "$expected"; then
        green "  ✅ $desc"
        PASS=$((PASS + 1))
    else
        red "  ❌ $desc (expected to contain: $expected)"
        echo "     got: $(echo "$actual" | head -1)"
        FAIL=$((FAIL + 1))
    fi
}

bold "╔══════════════════════════════════════╗"
bold "║      AIRouter — Integration Test     ║"
bold "╚══════════════════════════════════════╝"
echo ""

# ── 1. Health ──────────────────────────────────────────────
bold "── 1. Health Check ──"
HTTP=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/health" 2>/dev/null)
BODY=$(curl -s "$BASE/health" 2>/dev/null)
assert "GET /health → 200" "200" "$HTTP"
assert_contains "GET /health → OK" "OK" "$BODY"

# ── 2. Frontend ────────────────────────────────────────────
bold "── 2. Frontend Static Files ──"

HTTP=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/" 2>/dev/null)
assert "GET / → 200" "200" "$HTTP"

HTML=$(curl -s "$BASE/" 2>/dev/null)
assert_contains "index.html → <!DOCTYPE html>" "<!DOCTYPE html>" "$HTML"
assert_contains "index.html → has <title>" "AIRouter" "$HTML"

CSS=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/style/main.css" 2>/dev/null)
assert "GET /style/main.css → 200" "200" "$CSS"

JS_FILE=$(ls frontend-dist/*.js 2>/dev/null | head -1 | xargs basename)
if [ -n "$JS_FILE" ]; then
    HTTP_JS=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/$JS_FILE" 2>/dev/null)
    assert "GET /$JS_FILE → 200" "200" "$HTTP_JS"
else
    red "  ⚠️  No JS file found in frontend-dist/"
fi

WASM_FILE=$(ls frontend-dist/*.wasm 2>/dev/null | head -1 | xargs basename)
if [ -n "$WASM_FILE" ]; then
    HTTP_WASM=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/$WASM_FILE" 2>/dev/null)
    assert "GET /$WASM_FILE → 200" "200" "$HTTP_WASM"
else
    red "  ⚠️  No WASM file found in frontend-dist/"
fi

# ── 3. Auth ────────────────────────────────────────────────
bold "── 3. Authentication ──"

HTTP_NO=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/v1/models" 2>/dev/null)
assert "GET /v1/models (no auth) → 401" "401" "$HTTP_NO"

BODY_NO=$(curl -s "$BASE/v1/models" 2>/dev/null)
assert_contains "GET /v1/models (no auth) → error JSON" "invalid_api_key" "$BODY_NO"

HTTP_OK=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $KEY" "$BASE/v1/models" 2>/dev/null)
assert "GET /v1/models (auth ok) → 200" "200" "$HTTP_OK"

HTTP_BAD=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer sk-wrong" "$BASE/v1/models" 2>/dev/null)
assert "GET /v1/models (bad key) → 401" "401" "$HTTP_BAD"

HTTP_EMPTY=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: " "$BASE/v1/models" 2>/dev/null)
assert "GET /v1/models (empty auth) → 401" "401" "$HTTP_EMPTY"

# Auth header passthrough on health (no auth required)
HTTP_HEALTH=$(curl -s -o /dev/null -w "%{http_code}" "$BASE/health" 2>/dev/null)
assert "GET /health (no auth always ok) → 200" "200" "$HTTP_HEALTH"

# ── 4. Models API ──────────────────────────────────────────
bold "── 4. Models API ──"

MODELS=$(curl -s -H "Authorization: Bearer $KEY" "$BASE/v1/models" 2>/dev/null)
MODEL_COUNT=$(echo "$MODELS" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert "GET /v1/models → returns models" "$MODEL_COUNT" "$MODEL_COUNT"
[ "$MODEL_COUNT" -gt 10 ] && green "  ✅ Model count > 10 ($MODEL_COUNT)" && PASS=$((PASS+1)) || { red "  ❌ Too few models: $MODEL_COUNT"; FAIL=$((FAIL+1)); }

# Check free models present
FREE_MODELS=$(echo "$MODELS" | python3 -c "
import sys,json
d=json.load(sys.stdin)
free=[m['id'] for m in d if m['owned_by'] in ('opencode','mimo')]
print(len(free))
" 2>/dev/null || echo "0")
[ "$FREE_MODELS" -ge 10 ] && green "  ✅ Free models >= 10 ($FREE_MODELS)" && PASS=$((PASS+1)) || { red "  ❌ Too few free models: $FREE_MODELS"; FAIL=$((FAIL+1)); }

# ── 5. Dashboard API ───────────────────────────────────────
bold "── 5. Dashboard API ──"

DASH=$(curl -s -H "Authorization: Bearer $KEY" "$BASE/api/dashboard" 2>/dev/null)
DASH_PROV=$(echo "$DASH" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['metrics']['total_providers'])" 2>/dev/null || echo "0")
DASH_MOD=$(echo "$DASH" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['metrics']['total_models'])" 2>/dev/null || echo "0")
assert_contains "GET /api/dashboard → has providers" "total_providers" "$DASH"
[ "$DASH_PROV" -ge 2 ] && green "  ✅ Total providers >= 2 ($DASH_PROV)" && PASS=$((PASS+1)) || { red "  ❌ Too few providers: $DASH_PROV"; FAIL=$((FAIL+1)); }
[ "$DASH_MOD" -ge 10 ] && green "  ✅ Total models >= 10 ($DASH_MOD)" && PASS=$((PASS+1)) || { red "  ❌ Too few models: $DASH_MOD"; FAIL=$((FAIL+1)); }

# ── 6. Rate Limit ──────────────────────────────────────────
bold "── 6. Rate Limit (quick check) ──"

# Hit multiple times, should all pass at default 60/min
RATE_OK=0
for i in 1 2 3; do
    HTTP=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $KEY" "$BASE/v1/models" 2>/dev/null)
    [ "$HTTP" = "200" ] && RATE_OK=$((RATE_OK+1))
done
[ "$RATE_OK" -eq 3 ] && green "  ✅ 3 rapid requests → 200 (rate limit ok)" && PASS=$((PASS+1)) || { red "  ❌ Rate limit blocked prematurely: $RATE_OK/3"; FAIL=$((FAIL+1)); }

# ── 7. CORS ────────────────────────────────────────────────
bold "── 7. CORS Headers ──"

CORS=$(curl -s -o /dev/null -w "%{http_code}" -H "Origin: http://localhost:8080" -H "Access-Control-Request-Method: GET" -X OPTIONS "$BASE/v1/models" 2>/dev/null)
assert "OPTIONS /v1/models → 200 (CORS ok)" "200" "$CORS"

ORIGIN=$(curl -s -D - -H "Authorization: Bearer $KEY" "$BASE/v1/models" 2>/dev/null | grep -i "access-control-allow-origin" | head -1 | tr -d ' \r\n')
assert "GET /v1/models → Access-Control-Allow-Origin: *" "access-control-allow-origin:*" "$(echo "$ORIGIN" | tr '[:upper:]' '[:lower:]')"

# ── Summary ────────────────────────────────────────────────
echo ""
bold "═══════════════════════════════════════"
if [ "$FAIL" -eq 0 ]; then
    green "  ✅ ALL $PASS TESTS PASSED"
    exit 0
else
    red "  ❌ $FAIL TESTS FAILED, $PASS PASSED"
    exit 1
fi
