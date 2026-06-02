#!/usr/bin/env bash
# qemu/status.sh — 显示 QEMU 开发环境完整状态

set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/qemu/lib/log.sh"

CONTAINER_NAME="teaclave_dev_env"
BASE_URL="http://localhost:3000"

echo ""
echo "━━━ AirAccount QEMU 环境状态 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Docker 容器状态
printf "%-20s" "Docker 容器:"
if docker ps --format "{{.Names}}\t{{.Status}}" | grep -q "^$CONTAINER_NAME"; then
    STATUS=$(docker ps --format "{{.Status}}" --filter "name=$CONTAINER_NAME")
    echo -e "${GREEN}✓ 运行中${NC}  ($STATUS)"
else
    echo -e "${RED}✗ 未运行${NC}  (./qemu/setup.sh)"
fi

# QEMU 进程状态
printf "%-20s" "QEMU 进程:"
if docker exec "$CONTAINER_NAME" pgrep -x qemu-system-aarch64 &>/dev/null 2>/dev/null; then
    echo -e "${GREEN}✓ 运行中${NC}"
else
    echo -e "${YELLOW}○ 未运行${NC}  (./qemu/start.sh)"
fi

# KMS API 状态
printf "%-20s" "KMS API:"
if HEALTH=$(curl -sf --max-time 2 "$BASE_URL/health" 2>/dev/null); then
    echo -e "${GREEN}✓ 在线${NC}  $BASE_URL"
    VERSION=$(curl -sf "$BASE_URL/version" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('version','?'))" 2>/dev/null || echo "?")
    printf "%-20s%s\n" "  版本:" "v$VERSION"
else
    echo -e "${YELLOW}○ 离线${NC}  (./qemu/deploy.sh)"
fi

# 构建产物
printf "%-20s" "构建产物:"
TA_EXISTS=$(docker exec "$CONTAINER_NAME" test -f "/opt/teaclave/shared/ta/4319f351-0b24-4097-b659-80ee4f824cdd.ta" 2>/dev/null && echo "yes" || echo "no")
CA_EXISTS=$(docker exec "$CONTAINER_NAME" test -f "/opt/teaclave/shared/kms-api-server" 2>/dev/null && echo "yes" || echo "no")
[ "$TA_EXISTS" = "yes" ] && [ "$CA_EXISTS" = "yes" ] && echo -e "${GREEN}✓ TA + CA${NC}" || echo -e "${YELLOW}! 需要构建${NC}  (./qemu/build.sh)"

# 端口映射
echo ""
echo "  端口映射:"
echo "    localhost:3000  → QEMU guest:3000  (KMS API)"
echo "    localhost:54320 → QEMU guest UART0 (Linux shell)"
echo "    localhost:54321 → QEMU guest UART1 (OP-TEE log)"
echo "    localhost:54433 → QEMU guest:4433  (HTTPS test)"

# tmux 会话
echo ""
printf "%-20s" "tmux 会话:"
if tmux has-session -t "kms-qemu" 2>/dev/null; then
    echo -e "${GREEN}✓ kms-qemu${NC}  (tmux attach -t kms-qemu)"
else
    echo -e "${YELLOW}○ 无${NC}"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
