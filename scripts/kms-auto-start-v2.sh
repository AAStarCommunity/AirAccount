#!/bin/bash
# KMS 自动启动脚本 v2 - 不自动启动 API Server，保持串口可用
# 使用方式: ./scripts/kms-auto-start-v2.sh

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  KMS 启动脚本 v2 - 串口交互模式${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

echo -e "${YELLOW}🔄 停止旧的 QEMU 和监听器...${NC}"
docker exec teaclave_dev_env pkill -f qemu-system-aarch64 || true
docker exec teaclave_dev_env pkill -f listen_on_guest_vm_shell || true
docker exec teaclave_dev_env pkill -f listen_on_secure_world_log || true
docker exec teaclave_dev_env pkill -f "TCP-LISTEN:54320" || true
docker exec teaclave_dev_env pkill -f "TCP-LISTEN:54321" || true
docker exec teaclave_dev_env pkill -f "socat.*54320" || true
docker exec teaclave_dev_env pkill -f "socat.*54321" || true

# 使用 fuser 或直接 pkill（容器内可能没有 lsof）
# 如果端口仍被占用，pkill 已经处理了大部分情况
sleep 2

echo -e "${GREEN}✅ 清理完成${NC}"
echo ""

echo -e "${GREEN}🚀 步骤 1/4: 启动 Secure World 日志监听器（端口 54321）...${NC}"
docker exec -d teaclave_dev_env bash -c "socat TCP-LISTEN:54321,reuseaddr,fork -,raw,echo=0 > /dev/null 2>&1"
sleep 1
echo -e "   端口 54321 已准备好监听 TA 日志"
echo ""

echo -e "${GREEN}🚀 步骤 2/4: 启动 Guest VM 串口监听器（端口 54320）...${NC}"
echo -e "${YELLOW}   注意: 这次不会自动启动 API Server${NC}"
# 使用修改版的监听器：自动登录和挂载，但不启动 API Server
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell_no_api"
sleep 3
echo -e "   端口 54320 已准备好，Guest VM 将自动挂载 shared 目录（不启动 API）"
echo ""

echo -e "${GREEN}🚀 步骤 3/4: 启动 QEMU（带 3000 端口转发）...${NC}"
docker exec -d teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8 > /tmp/qemu.log 2>&1"

echo -e "${YELLOW}⏳ 等待 30 秒让 QEMU 启动完成...${NC}"
sleep 30

echo ""
echo -e "${GREEN}✅ QEMU 启动完成！${NC}"
echo ""

# 验证 QEMU 是否运行
if docker exec teaclave_dev_env pgrep -f qemu-system-aarch64 > /dev/null; then
    echo -e "${GREEN}✅ QEMU 进程正在运行${NC}"
    docker exec teaclave_dev_env ps aux | grep qemu | grep -o "hostfwd=[^[:space:]]*" || echo "   (未找到端口转发信息)"
else
    echo -e "${YELLOW}⚠️  未检测到 QEMU 进程，请检查日志${NC}"
fi

echo ""
echo -e "${GREEN}🚀 步骤 4/4: 启动 Cloudflare Tunnel...${NC}"

# 检查是否已有 cloudflared 进程运行
if pgrep -f "cloudflared tunnel run kms-tunnel" > /dev/null; then
    EXISTING_PIDS=$(pgrep -f "cloudflared tunnel run kms-tunnel")
    echo -e "${YELLOW}⚠️  发现已运行的 cloudflared 进程: $EXISTING_PIDS${NC}"
    echo -e "${YELLOW}   正在停止旧进程...${NC}"
    pkill -f "cloudflared tunnel run kms-tunnel" || true
    sleep 2
fi

# 启动 cloudflared tunnel
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &
TUNNEL_PID=$!
sleep 3

# 验证 tunnel 是否启动成功
if ps -p $TUNNEL_PID > /dev/null; then
    echo -e "${GREEN}✅ Cloudflare Tunnel 已启动 (PID: $TUNNEL_PID)${NC}"
else
    echo -e "${YELLOW}⚠️  Tunnel 启动可能失败，请检查日志: tail -f /tmp/cloudflared.log${NC}"
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  使用说明${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${GREEN}1. 监控 CA (Client Application) 日志:${NC}"
echo -e "   ./scripts/kms-qemu-terminal2-enhanced.sh"
echo ""
echo -e "${GREEN}2. 监控 TA (Trusted Application) 日志:${NC}"
echo -e "   ./scripts/kms-qemu-terminal3-v2.sh"
echo -e "   ${YELLOW}(v2 版本直接连接已有监听器，避免端口冲突)${NC}"
echo ""
echo -e "${GREEN}3. 使用交互式工具管理 Guest VM:${NC}"
echo -e "   ./scripts/kms-guest-interactive.sh"
echo -e "   ${YELLOW}(推荐) 提供菜单式操作：启动/停止 API、执行命令、部署 TA 等${NC}"
echo ""
echo -e "${GREEN}4. 进入 Guest VM 直接 Shell (高级):${NC}"
echo -e "   docker exec -it teaclave_dev_env socat STDIN TCP:localhost:54320"
echo -e "   ${YELLOW}直接连接到 Guest VM 串口（需要手动输入命令）${NC}"
echo ""
echo -e "${GREEN}5. 执行单个命令（脚本方式）:${NC}"
echo -e "   ./scripts/kms-guest-exec.sh \"cd /root/shared && ls -la\""
echo ""
echo -e "${GREEN}6. 快速启动 API Server:${NC}"
echo -e "   echo 'cd /root/shared && nohup ./kms_ca > api.log 2>&1 &' | docker exec -i teaclave_dev_env socat - TCP:localhost:54320"
echo -e "   ${YELLOW}然后等待 15 秒，测试: curl http://localhost:3000/health${NC}"
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}🎉 KMS 已启动完成（交互模式）${NC}"
echo -e "${BLUE}========================================${NC}"
