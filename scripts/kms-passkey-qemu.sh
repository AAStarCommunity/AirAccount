#!/bin/bash
# KMS Passkey QEMU 管理脚本

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

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "\n${BLUE}==>${NC} $1\n"
}

CONTAINER_NAME="kms_passkey_dev"

# 检查容器是否运行
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    log_error "容器未运行！请先执行: ./scripts/kms-passkey-docker.sh start"
    exit 1
fi

# 启动 QEMU
start_qemu() {
    log_step "启动 KMS Passkey QEMU 环境"
    log_warn "使用端口: 54330 (VM Shell), 54331 (Secure Log)"
    log_info "共享目录: /opt/kms_passkey/shared"
    
    echo ""
    log_info "⚠️  请在其他终端先启动监控:"
    log_info "   Terminal 2 (Guest VM): ./scripts/kms-passkey-monitor-vm.sh"
    log_info "   Terminal 3 (Secure World): ./scripts/kms-passkey-monitor-secure.sh"
    echo ""
    log_info "3秒后启动 QEMU..."
    sleep 3
    
    docker exec -it "$CONTAINER_NAME" bash -c "
        cd /root/teaclave_sdk_src
        IMG_DIRECTORY=/opt/teaclave/images \
        IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory \
        QEMU_HOST_SHARE_DIR=/opt/kms_passkey/shared \
        LISTEN_MODE=ON \
        ./scripts/runtime/bin/start_qemuv8
    "
}

# 连接到 Guest VM Shell
shell_vm() {
    log_info "连接到 Guest VM Shell (端口 54330)..."
    docker exec -it "$CONTAINER_NAME" bash -c "
        socat - TCP:localhost:54320,crlf
    "
}

# 查看 Secure World 日志
logs_secure() {
    log_info "查看 Secure World 日志 (端口 54331)..."
    docker exec -it "$CONTAINER_NAME" bash -c "
        socat - TCP:localhost:54321
    "
}

# 停止 QEMU
stop_qemu() {
    log_info "停止 QEMU..."
    docker exec "$CONTAINER_NAME" bash -c "
        pkill -f qemu-system-aarch64 || true
    "
    log_info "✅ QEMU 已停止"
}

# 查看状态
status() {
    log_step "KMS Passkey QEMU 状态"
    
    docker exec "$CONTAINER_NAME" bash -c "
        if pgrep -f qemu-system-aarch64 > /dev/null; then
            echo '✅ QEMU 正在运行'
            echo ''
            echo 'QEMU 进程:'
            ps aux | grep qemu-system-aarch64 | grep -v grep
        else
            echo '❌ QEMU 未运行'
        fi
        
        echo ''
        echo '共享目录内容:'
        ls -lh /opt/kms_passkey/shared/ 2>/dev/null || echo '共享目录为空'
    "
}

# 主命令分发
case "${1:-}" in
    start)
        start_qemu
        ;;
    stop)
        stop_qemu
        ;;
    shell|vm)
        shell_vm
        ;;
    logs|secure)
        logs_secure
        ;;
    status)
        status
        ;;
    *)
        echo "Usage: $0 {start|stop|shell|logs|status}"
        echo ""
        echo "Commands:"
        echo "  start   - Start QEMU emulator"
        echo "  stop    - Stop QEMU emulator"
        echo "  shell   - Connect to Guest VM shell (port 54330)"
        echo "  logs    - View Secure World logs (port 54331)"
        echo "  status  - Show QEMU status"
        exit 1
        ;;
esac
