#!/bin/bash
# 清理所有 KMS 相关进程，为手动启动做准备

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}🧹 清理 KMS 相关进程...${NC}"
echo ""

# 停止 QEMU
echo -n "  停止 QEMU... "
docker exec teaclave_dev_env pkill -f qemu-system-aarch64 2>/dev/null && echo -e "${GREEN}✓${NC}" || echo -e "${YELLOW}(未运行)${NC}"

# 停止 socat
echo -n "  停止 socat... "
docker exec teaclave_dev_env pkill -f "TCP-LISTEN:54320" 2>/dev/null
docker exec teaclave_dev_env pkill -f "TCP-LISTEN:54321" 2>/dev/null
docker exec teaclave_dev_env pkill -f socat 2>/dev/null && echo -e "${GREEN}✓${NC}" || echo -e "${YELLOW}(未运行)${NC}"

# 停止监听器
echo -n "  停止监听器... "
docker exec teaclave_dev_env pkill -f listen_on_guest_vm_shell 2>/dev/null
docker exec teaclave_dev_env pkill -f listen_on_secure_world_log 2>/dev/null && echo -e "${GREEN}✓${NC}" || echo -e "${YELLOW}(未运行)${NC}"

# 清理僵尸进程
echo -n "  清理僵尸进程... "
zombie_count=$(docker exec teaclave_dev_env ps aux | grep defunct | grep -v grep | wc -l)
if [ "$zombie_count" -gt 0 ]; then
    echo -e "${YELLOW}发现 $zombie_count 个僵尸进程${NC}"
    echo -e "    ${YELLOW}(僵尸进程会在父进程退出时自动清理)${NC}"
else
    echo -e "${GREEN}✓${NC}"
fi

sleep 2

echo ""
echo -e "${GREEN}✅ 清理完成！${NC}"
echo ""
echo -e "${YELLOW}现在可以选择：${NC}"
echo -e "  ${GREEN}1)${NC} 一键启动: ${GREEN}./scripts/kms-auto-start.sh${NC}"
echo -e "  ${GREEN}2)${NC} 手动三终端启动:"
echo -e "     终端1: ${GREEN}./scripts/terminal3-secure-log.sh${NC}"
echo -e "     终端2: ${GREEN}./scripts/terminal2-guest-vm.sh${NC}"
echo -e "     终端3: ${GREEN}./scripts/terminal1-qemu.sh${NC}"
echo ""
