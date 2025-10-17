#!/bin/bash
# KMS完全清理并重启脚本
# 目的：确保使用最新编译的二进制，清除所有旧状态

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_step() {
    echo -e "\n${BLUE}==>${NC} $1"
}

# Step 1: 清理所有旧进程
log_step "1/5 清理所有旧进程..."
docker exec teaclave_dev_env bash -c "pkill -9 qemu-system || true"
docker exec teaclave_dev_env bash -c "pkill -9 socat || true"
docker exec teaclave_dev_env bash -c "pkill -9 expect || true"
sleep 2
log_info "✅ 进程已清理"

# Step 2: 清理状态文件（保留二进制）
log_step "2/5 清理状态文件..."
docker exec teaclave_dev_env bash -c "
    cd /opt/teaclave/shared
    rm -f address_map.json kms-api.log *.log 2>/dev/null || true
    ls -lh | grep -E 'kms|\.ta$'
"
log_info "✅ 状态文件已清理"

# Step 3: 验证最新二进制存在
log_step "3/5 验证最新二进制..."
TIMESTAMP=$(docker exec teaclave_dev_env stat -c "%y" /opt/teaclave/shared/kms-api-server)
log_info "kms-api-server 时间戳: $TIMESTAMP"

# Step 4: 创建新的 rootfs（可选，如果需要清理 QEMU 内的旧文件）
log_step "4/5 准备 QEMU 启动..."
log_info "QEMU 将从 /root/shared 加载新二进制（挂载自 /opt/teaclave/shared）"

# Step 5: 启动服务（使用后台 expect）
log_step "5/5 启动服务..."
log_info "启动 Secure World 监听器..."
docker exec -d teaclave_dev_env expect /opt/teaclave/bin/listen_on_secure_world_log

sleep 2

log_info "启动 Guest VM 监听器..."
docker exec -d teaclave_dev_env expect /opt/teaclave/bin/listen_on_guest_vm_shell

sleep 3

log_info "启动 QEMU..."
docker exec -d teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=ON ./scripts/runtime/bin/start_qemuv8"

log_info "等待 60 秒让服务完全启动..."
for i in {1..12}; do
    echo -n "."
    sleep 5
done
echo ""

# Step 6: 验证
log_step "验证部署..."
log_info "检查进程状态..."
docker exec teaclave_dev_env ps aux | grep -E "qemu|expect" | grep -v grep || log_warn "未找到 QEMU/expect 进程"

log_info "测试 API..."
if curl -s -m 5 'http://localhost:3000/health' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ API 服务响应正常${NC}"
    curl -s 'http://localhost:3000/health' | jq .
else
    echo -e "${RED}❌ API 服务无响应，请检查日志${NC}"
    echo "调试命令:"
    echo "  docker exec teaclave_dev_env ps aux | grep qemu"
    echo "  docker exec teaclave_dev_env ps aux | grep expect"
    exit 1
fi

echo ""
log_info "🎉 重启完成！"
echo ""
echo -e "${BLUE}测试命令：${NC}"
echo '  curl -s -X POST http://localhost:3000/CreateKey -H "Content-Type: application/x-amz-json-1.1" -H "x-amz-target: TrentService.CreateKey" -d '"'"'{"Description":"Test"}'"'"' | jq .'
echo ""
