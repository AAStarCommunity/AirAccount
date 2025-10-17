#!/bin/bash
# KMS 完全自动启动脚本

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}🔄 停止旧的 QEMU 和监听器...${NC}"
docker exec teaclave_dev_env pkill -f qemu-system-aarch64 || true
docker exec teaclave_dev_env pkill -f listen_on_guest_vm_shell || true
docker exec teaclave_dev_env pkill -f listen_on_secure_world_log || true
docker exec teaclave_dev_env pkill -f "TCP-LISTEN:54320" || true
docker exec teaclave_dev_env pkill -f "TCP-LISTEN:54321" || true

# 强制杀掉占用端口的进程
docker exec teaclave_dev_env bash -c "lsof -ti:54320 | xargs -r kill -9 2>/dev/null || true"
docker exec teaclave_dev_env bash -c "lsof -ti:54321 | xargs -r kill -9 2>/dev/null || true"
sleep 2

echo -e "${GREEN}🚀 启动 Secure World 监听器（端口 54321）...${NC}"
docker exec -d teaclave_dev_env bash -c "socat TCP-LISTEN:54321,reuseaddr,fork -,raw,echo=0 > /dev/null 2>&1"
sleep 1

echo -e "${GREEN}🚀 启动 Guest VM 监听脚本（端口 54320）...${NC}"
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"
sleep 3

echo -e "${GREEN}🖥️  启动 QEMU（带 3000 端口转发）...${NC}"
docker exec -d teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8 > /tmp/qemu.log 2>&1"

echo -e "${YELLOW}⏳ 等待 45 秒让 QEMU 和 API Server 启动...${NC}"
sleep 45

echo -e "${GREEN}✅ 验证端口转发配置...${NC}"
docker exec teaclave_dev_env ps aux | grep qemu | grep -o "hostfwd=[^[:space:]]*"

echo ""
echo -e "${GREEN}✅ 测试 Mac 本地访问...${NC}"
curl -s http://localhost:3000/health | jq .

echo ""
echo -e "${GREEN}✅ 所有服务已启动！${NC}"
echo -e "   Mac 本地: ${GREEN}http://localhost:3000${NC}"
echo -e "   公网访问: ${GREEN}https://kms.aastar.io${NC}"
