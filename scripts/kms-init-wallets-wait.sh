#!/bin/bash
# KMS 钱包初始化工具 - 等待 API Server 启动后自动初始化

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo -e "${BLUE}🔄 KMS 钱包初始化工具${NC}"
echo ""

echo -e "${YELLOW}⏳ 等待 API Server 启动...${NC}"
echo "   (按 Ctrl+C 取消等待)"
echo ""

# 等待 API Server 就绪
WAIT_COUNT=0
while true; do
    if curl -s http://localhost:3000/health > /dev/null 2>&1; then
        echo ""
        echo -e "${GREEN}✅ API Server 已就绪${NC}"
        break
    fi

    WAIT_COUNT=$((WAIT_COUNT + 1))
    if [ $((WAIT_COUNT % 10)) -eq 0 ]; then
        echo -n "."
    fi
    sleep 1

    # 每 30 秒显示一次提示
    if [ $((WAIT_COUNT % 30)) -eq 0 ]; then
        echo ""
        echo -e "${YELLOW}仍在等待... (已等待 ${WAIT_COUNT} 秒)${NC}"
        echo -e "${YELLOW}提示: 确保已启动 Terminal 3 → 2 → 1${NC}"
    fi
done

echo ""
echo -e "${GREEN}🔄 初始化开发测试钱包...${NC}"
echo ""

# 运行钱包初始化
"$SCRIPT_DIR/kms-init-dev-wallets.sh"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}✅ 钱包初始化完成！${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "${BLUE}📋 查看钱包:${NC}"
echo "  curl -s -X POST http://localhost:3000/ListKeys \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -H 'x-amz-target: TrentService.ListKeys' \\"
echo "    -d '{}' | jq ."
echo ""
echo -e "${BLUE}🧪 测试签名:${NC}"
echo "  curl -s -X POST http://localhost:3000/Sign \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"KeyId\":\"<wallet-id>\",\"Message\":\"0x68656c6c6f\"}' | jq ."
echo ""
