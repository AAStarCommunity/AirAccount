#!/bin/bash
# KMS 日志监控脚本 - 在 auto-start 后使用

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}🔍 KMS 服务监控${NC}"
echo ""

# 检查 QEMU 是否运行
if ! docker exec teaclave_dev_env ps aux | grep -q "[q]emu-system-aarch64"; then
    echo -e "${YELLOW}❌ QEMU 未运行！${NC}"
    echo -e "请先运行: ${GREEN}./scripts/kms-auto-start.sh${NC}"
    exit 1
fi

echo -e "${GREEN}✅ QEMU 正在运行${NC}"
echo ""
echo -e "${YELLOW}选择监控模式：${NC}"
echo "  1) Secure World 日志 (TA)"
echo "  2) Guest VM Shell (CA)"
echo "  3) 查看 QEMU 日志"
echo "  4) 查看 API Server 日志"
echo "  5) 查看 cloudflared 日志"
echo ""
read -p "请选择 [1-5]: " choice

case $choice in
    1)
        echo -e "${GREEN}🔒 连接到 Secure World 日志...${NC}"
        docker exec -it teaclave_dev_env bash -c "socat - TCP:localhost:54321"
        ;;
    2)
        echo -e "${GREEN}🖥️  连接到 Guest VM Shell...${NC}"
        docker exec -it teaclave_dev_env bash -c "socat - TCP:localhost:54320"
        ;;
    3)
        echo -e "${GREEN}📋 QEMU 日志：${NC}"
        docker exec teaclave_dev_env tail -100 /tmp/qemu.log
        ;;
    4)
        echo -e "${GREEN}📋 API Server 日志：${NC}"
        echo -e "${YELLOW}提示: 日志在 QEMU Guest 内，通过 Guest VM Shell 查看${NC}"
        echo -e "运行命令: ${GREEN}cat /root/shared/kms-api.log${NC}"
        docker exec -it teaclave_dev_env bash -c "echo 'cat /root/shared/kms-api.log' | socat - TCP:localhost:54320"
        ;;
    5)
        echo -e "${GREEN}📋 Cloudflared 日志（最近 50 行）：${NC}"
        tail -50 /tmp/cloudflared.log
        ;;
    *)
        echo "无效选择"
        exit 1
        ;;
esac
