#!/bin/bash
# 重启 QEMU 内的 KMS API Server

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}🔄 重启 KMS API Server...${NC}"

# 检查 QEMU 是否运行
if ! docker exec teaclave_dev_env ps aux | grep -q "[q]emu-system-aarch64"; then
    echo -e "${RED}❌ QEMU 未运行！${NC}"
    echo -e "${YELLOW}请先运行: ./scripts/kms-auto-start.sh${NC}"
    exit 1
fi

# 创建重启脚本
docker exec teaclave_dev_env bash -c "cat > /opt/teaclave/shared/.restart_api.sh << 'EOF'
#!/bin/sh
pkill kms-api-server 2>/dev/null || true
sleep 1
cd /root/shared
./kms-api-server > kms-api.log 2>&1 &
sleep 2
ps aux | grep kms-api-server | grep -v grep && echo 'API Server restarted successfully'
EOF
chmod +x /opt/teaclave/shared/.restart_api.sh"

# 执行重启
echo -e "${GREEN}执行重启命令...${NC}"
docker exec teaclave_dev_env bash -c "echo 'cd /root/shared && sh .restart_api.sh' | socat - TCP:localhost:54320" 2>/dev/null || {
    echo -e "${YELLOW}⚠️  自动重启失败，可能是 socat 连接超时${NC}"
    echo -e "${YELLOW}请手动连接 QEMU 并执行：${NC}"
    echo -e "  ${GREEN}pkill kms-api-server${NC}"
    echo -e "  ${GREEN}cd /root/shared && ./kms-api-server > kms-api.log 2>&1 &${NC}"
    exit 1
}

echo ""
echo -e "${GREEN}✅ API Server 重启完成！${NC}"
echo ""
echo -e "${GREEN}测试命令：${NC}"
echo -e "  curl http://localhost:3000/health"
