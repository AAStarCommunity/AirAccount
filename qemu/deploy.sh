#!/usr/bin/env bash
# qemu/deploy.sh — 部署 TA + CA 到正在运行的 QEMU guest
#
# QEMU guest 必须已经启动（通过 start.sh）并登录。
# 通过 socat 连接 guest shell（port 54320）发送命令。
#
# 用法：
#   ./qemu/deploy.sh          # 部署 TA + CA，启动 kms-api-server
#   ./qemu/deploy.sh restart  # 仅重启 kms-api-server（不重新加载 TA）
#   ./qemu/deploy.sh ta       # 仅重新加载 TA（需要重启 tee-supplicant）

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/qemu/lib/log.sh"

CONTAINER_NAME="teaclave_dev_env"
TA_UUID="4319f351-0b24-4097-b659-80ee4f824cdd"
KMS_DB_PATH="/data/kms/kms.db"
GUEST_PORT=54320

# 在 QEMU guest 内执行命令（通过 socat）
guest_exec() {
    local cmd="$1"
    log_debug "guest: $cmd"
    docker exec "$CONTAINER_NAME" bash -c \
        "echo '$cmd' | socat - TCP:localhost:${GUEST_PORT}" 2>/dev/null || {
        log_error "无法连接 QEMU guest (port $GUEST_PORT)。QEMU 是否已启动？"
        return 1
    }
}

check_qemu_running() {
    if ! docker exec "$CONTAINER_NAME" bash -c \
        "timeout 2 socat - TCP:localhost:${GUEST_PORT},crnl </dev/null" 2>/dev/null; then
        log_error "QEMU guest 未响应。请先执行: ./qemu/start.sh"
        exit 1
    fi
}

mount_shared() {
    log_step "挂载 9p 共享目录"
    docker exec "$CONTAINER_NAME" bash -c "echo 'mkdir -p /root/shared && mount -t 9p -o trans=virtio host /root/shared 2>/dev/null || true' | socat - TCP:localhost:54320"
}

deploy_ta() {
    log_step "部署 TA 到 /lib/optee_armtz/"
    # 通过 socat 在 guest 内执行
    for cmd in \
        "mkdir -p /lib/optee_armtz" \
        "mount --bind /root/shared/ta /lib/optee_armtz 2>/dev/null || true" \
        "cp /root/shared/ta/${TA_UUID}.ta /lib/optee_armtz/" \
        "pkill tee-supplicant 2>/dev/null; sleep 1; tee-supplicant &" \
        "sleep 2 && echo 'TA deployed'"; do
        guest_exec "$cmd"
    done
    log_info "TA 已部署"
}

deploy_ca() {
    log_step "启动 kms-api-server"
    for cmd in \
        "mkdir -p /data/kms" \
        "pkill kms-api-server 2>/dev/null; sleep 1; true" \
        "KMS_DB_PATH=${KMS_DB_PATH} /root/shared/kms-api-server > /tmp/kms.log 2>&1 &" \
        "sleep 3 && curl -s http://localhost:3000/health || echo 'waiting...'"; do
        guest_exec "$cmd"
    done
    log_info "kms-api-server 已启动"
}

verify_deployment() {
    log_step "验证部署"
    sleep 2
    if curl -sf http://localhost:3000/health 2>/dev/null; then
        log_info "✓ KMS API 正常响应"
        curl -s http://localhost:3000/version 2>/dev/null || true
    else
        log_warn "KMS API 未就绪，查看日志："
        docker exec "$CONTAINER_NAME" bash -c \
            "echo 'tail -20 /tmp/kms.log' | socat - TCP:localhost:54320" 2>/dev/null || true
    fi
}

case "${1:-all}" in
    all)
        check_qemu_running
        mount_shared
        deploy_ta
        deploy_ca
        verify_deployment
        log_info "部署完成 ✓  API: http://localhost:3000/health"
        ;;
    restart)
        check_qemu_running
        deploy_ca
        verify_deployment
        ;;
    ta)
        check_qemu_running
        deploy_ta
        ;;
    *)
        echo "用法: $0 [all|restart|ta]"
        exit 1 ;;
esac
