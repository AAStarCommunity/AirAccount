#!/bin/bash
# Step 3: Run all tests (unit + API + performance)
# Usage: ./run-all-tests.sh [host:port] [perf-rounds]
# Default: 192.168.7.2:3000, 5 rounds

set -eo pipefail

HOST="${1:-192.168.7.2:3000}"
ROUNDS="${2:-5}"
YELLOW='\033[1;33m'; GREEN='\033[0;32m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'; BOLD='\033[1m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
KMS_DIR="$(dirname "$SCRIPT_DIR")"
TEST_DIR="$KMS_DIR/test"
OUTPUT_FILE="$KMS_DIR/../full-test-result-3-3-2026.md"

echo "${BOLD}================================================================${NC}"
echo "${BOLD}  KMS Full Test Suite${NC}"
echo "${BOLD}  $(date '+%Y-%m-%d %H:%M:%S')${NC}"
echo "${BOLD}================================================================${NC}"
echo ""

# Initialize output file
cat > "$OUTPUT_FILE" << 'HEADER'
# KMS Full Test Results

Date: $(date '+%Y-%m-%d %H:%M:%S')
Board: STM32MP157F-DK2 (Cortex-A7 650MHz)
Branch: KMS-stm32

HEADER
date "+Date: %Y-%m-%d %H:%M:%S" > "$OUTPUT_FILE"
cat >> "$OUTPUT_FILE" << 'HEADER'
Board: STM32MP157F-DK2 (Cortex-A7 650MHz)
Branch: KMS-stm32

HEADER

# ── Phase 1: Unit Tests ──
echo "${YELLOW}[1/3] Unit Tests${NC}"
echo ""

echo "--- Proto crate ---"
PROTO_RESULT=$(cd "$KMS_DIR/proto" && cargo test 2>&1)
PROTO_PASS=$(echo "$PROTO_RESULT" | grep "test result:" | grep -o "[0-9]* passed" | head -1)
echo "  proto: $PROTO_PASS"

echo "--- Host crate (lib) ---"
HOST_RESULT=$(cd "$KMS_DIR/host" && cargo test --no-default-features --lib 2>&1)
HOST_PASS=$(echo "$HOST_RESULT" | grep "test result:" | grep -o "[0-9]* passed" | head -1)
echo "  host:  $HOST_PASS"

echo "" >> "$OUTPUT_FILE"
echo "## Unit Tests" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"
echo "| Crate | Tests | Status |" >> "$OUTPUT_FILE"
echo "|-------|-------|--------|" >> "$OUTPUT_FILE"
echo "| proto | $PROTO_PASS | PASS |" >> "$OUTPUT_FILE"
echo "| host (lib) | $HOST_PASS | PASS |" >> "$OUTPUT_FILE"
echo "" >> "$OUTPUT_FILE"

echo ""
echo "${GREEN}Unit tests passed${NC}"
echo ""

# ── Phase 2: API Tests ──
echo "${YELLOW}[2/3] API Tests (target: $HOST)${NC}"
echo ""

if curl -s --max-time 5 "http://$HOST/health" > /dev/null 2>&1; then
    API_RESULT=$("$TEST_DIR/run-api-tests.sh" "$HOST" 2>&1) || true
    echo "$API_RESULT"

    echo "## API Tests" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo '```' >> "$OUTPUT_FILE"
    echo "$API_RESULT" | sed 's/\x1b\[[0-9;]*m//g' >> "$OUTPUT_FILE"
    echo '```' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
else
    echo "${RED}  DK2 not reachable at $HOST, skipping API tests${NC}"
    echo "## API Tests" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "SKIPPED: DK2 not reachable at $HOST" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
fi

echo ""

# ── Phase 3: Performance Tests ──
echo "${YELLOW}[3/3] Performance Benchmark ($ROUNDS rounds, target: $HOST)${NC}"
echo ""

if curl -s --max-time 5 "http://$HOST/health" > /dev/null 2>&1; then
    PERF_RESULT=$("$TEST_DIR/perf-test.sh" "$HOST" "$ROUNDS" 2>&1) || true
    echo "$PERF_RESULT"

    echo "## Performance Benchmark" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    # Extract markdown table from perf output
    echo "$PERF_RESULT" | sed -n '/| Operation/,/^$/p' | sed 's/\x1b\[[0-9;]*m//g' >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
else
    echo "${RED}  DK2 not reachable at $HOST, skipping performance tests${NC}"
    echo "## Performance Benchmark" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
    echo "SKIPPED: DK2 not reachable at $HOST" >> "$OUTPUT_FILE"
    echo "" >> "$OUTPUT_FILE"
fi

echo ""
echo "${BOLD}================================================================${NC}"
echo "${BOLD}  Results saved to: ${CYAN}$OUTPUT_FILE${NC}"
echo "${BOLD}================================================================${NC}"
