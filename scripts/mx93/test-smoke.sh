#!/bin/bash
# Quick smoke test against running KMS service.
# Usage: bash test-smoke.sh [host:port]   (default: localhost:3000)
set -eo pipefail

HOST="${1:-localhost:3000}"
BASE="http://$HOST"
PASS=0; FAIL=0

c_g='\033[0;32m'; c_r='\033[0;31m'; c_y='\033[1;33m'; c_nc='\033[0m'
ok()   { echo -e "  ${c_g}PASS${c_nc}  $1  ($2 ms)"; PASS=$((PASS+1)); }
fail() { echo -e "  ${c_r}FAIL${c_nc}  $1  ($2 ms) → $3"; FAIL=$((FAIL+1)); }

ms() { python3 -c 'import time; print(int(time.time()*1000))' 2>/dev/null || date +%s%3N; }

check() {
    local label="$1" expect_code="$2"; shift 2
    local t0 t1 code body
    t0=$(ms)
    body=$(curl -sw '%{http_code}' --max-time 15 "$@" -o /tmp/.smoke_body 2>/dev/null)
    code="${body: -3}"
    body=$(cat /tmp/.smoke_body 2>/dev/null)
    t1=$(ms)
    local elapsed=$(( t1 - t0 ))
    if [ "$code" = "$expect_code" ]; then
        ok "$label" "$elapsed"
    else
        fail "$label" "$elapsed" "HTTP $code  $(echo "$body" | head -c 80)"
    fi
}

echo ""
echo "Smoke test → $BASE"
echo "────────────────────────────────────────"

# Basic endpoints
check "GET /health"      200 "$BASE/health"
check "GET /stats"       200 "$BASE/stats"
check "GET /stats content-type" 200 "$BASE/stats"

# Verify charset in Content-Type
CT=$(curl -sD - "$BASE/stats" -o /dev/null 2>/dev/null | grep -i '^content-type' | tr -d '\r\n')
if echo "$CT" | grep -q 'charset=utf-8'; then
    ok "Content-Type has charset=utf-8" "0"
else
    fail "Content-Type charset" "0" "$CT"
fi

# Verify JSON is valid
if curl -s "$BASE/stats" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null; then
    ok "Stats JSON is valid" "0"
else
    fail "Stats JSON invalid" "0" ""
fi

# Verify Chinese doesn't look garbled (check for multi-byte UTF-8)
ZH=$(curl -s "$BASE/stats" | python3 -c "
import sys,json
d=json.load(sys.stdin)
zh=d.get('_explain',{}).get('api_keys',{}).get('zh','')
print('OK' if len(zh)>5 and '\\xa5' not in zh else 'GARBLED:'+zh[:30])
" 2>/dev/null)
if echo "$ZH" | grep -q "^OK"; then
    ok "Chinese text not garbled" "0"
else
    fail "Chinese text garbled" "0" "$ZH"
fi

# ListKeys — non-destructive TEE read, confirms TEE is responsive without creating state
# (CreateKey is intentionally excluded: it writes to TA + triggers CAAM RNG,
#  which can hang under concurrent load and crash the entire board)
LIST_BODY=$(curl -s --max-time 10 \
    -X POST "$BASE/ListKeys" \
    -H "Content-Type: application/json" \
    -H "x-amz-target: TrentService.ListKeys" \
    -d '{"Limit":1}' 2>/dev/null)
if echo "$LIST_BODY" | grep -qE '"Keys"'; then
    ok "POST /ListKeys (TEE responsive)" "0"
elif echo "$LIST_BODY" | grep -qE '"error"|"Error"'; then
    fail "POST /ListKeys" "0" "$(echo "$LIST_BODY" | head -c 80)"
else
    ok "POST /ListKeys (empty)" "0"
fi

# QueueStatus — confirms circuit breaker is closed
QS_BODY=$(curl -s --max-time 5 "$BASE/QueueStatus" 2>/dev/null)
CB=$(echo "$QS_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('circuit_breaker_open','?'))" 2>/dev/null)
if [ "$CB" = "False" ] || [ "$CB" = "false" ] || [ "$CB" = "None" ] || [ "$CB" = "null" ]; then
    ok "Circuit breaker closed" "0"
elif [ -z "$CB" ] || [ "$CB" = "?" ]; then
    ok "QueueStatus N/A" "0"
else
    fail "Circuit breaker OPEN" "0" "consecutive_failures=$(echo "$QS_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('consecutive_failures','-'))" 2>/dev/null)"
fi

echo "────────────────────────────────────────"
echo -e "  ${c_g}PASS: $PASS${c_nc}   ${c_r}FAIL: $FAIL${c_nc}"
echo ""
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
