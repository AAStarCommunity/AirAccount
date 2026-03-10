#!/bin/bash
# KMS 私钥导出指南 - 显示如何从 Guest VM 导出私钥

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

WALLET_ID="${1:-<wallet-id>}"

echo -e "${BLUE}🔑 KMS 私钥导出指南${NC}"
echo ""
echo -e "${GREEN}方式 1: 直接连接 Guest VM（推荐）${NC}"
echo ""
echo "1. 连接到 Guest VM shell:"
echo "   ${YELLOW}docker exec -it teaclave_dev_env socat STDIN TCP:localhost:54320${NC}"
echo ""
echo "2. 在 Guest VM 中执行:"
echo "   ${YELLOW}cd /root/shared${NC}"
echo "   ${YELLOW}./export_key $WALLET_ID \"m/44'/60'/0'/0/0\"${NC}"
echo ""
echo "3. 按 Ctrl+C 退出"
echo ""
echo -e "${GREEN}方式 2: 通过 kms-guest-interactive.sh${NC}"
echo ""
echo "   ${YELLOW}./scripts/kms-guest-interactive.sh${NC}"
echo "   选择选项 7 (执行自定义命令)"
echo "   输入: ${YELLOW}./export_key $WALLET_ID \"m/44'/60'/0'/0/0\"${NC}"
echo ""
echo -e "${GREEN}方式 3: 一次性命令${NC}"
echo ""
echo "   ${YELLOW}echo 'cd /root/shared && ./export_key $WALLET_ID \"m/44\\\\\\''/60\\\\\\''/0\\\\\\''/0/0\"' | docker exec -i teaclave_dev_env socat - TCP:localhost:54320${NC}"
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}获取钱包列表${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "  curl -s -X POST http://localhost:3000/ListKeys \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -H 'x-amz-target: TrentService.ListKeys' \\"
echo "    -d '{}' | jq -r '.Keys[] | \"\\(.KeyId) - \\(.KeyMetadata.Address)\"'"
echo ""
