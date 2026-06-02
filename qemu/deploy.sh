#!/usr/bin/env bash
# qemu/deploy.sh — 部署 TA + CA 到正在运行的 QEMU guest
#
# QEMU guest 必须已经启动（通过 start.sh）并完成 boot (~90s)。
# 通过 Unix socket (/tmp/qemu-normal.sock) 连接 guest shell 发送命令。
# TA/CA 二进制通过临时 HTTP 服务器传输（9p 在 OrbStack 下不可用）。
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
SHARED_DIR="/opt/teaclave/shared"
HTTP_PORT=8877

# 在 QEMU guest 内执行命令（通过 Unix socket socat）
guest_exec() {
    local cmd="$1"
    local timeout="${2:-10}"
    log_debug "guest: $cmd"
    docker exec "$CONTAINER_NAME" bash -c "
        (sleep 1; printf '%s\n' $(printf '%q' "$cmd"); sleep $timeout) | socat - UNIX:/tmp/qemu-normal.sock 2>/dev/null
    "
}

check_qemu_running() {
    log_step "检查 QEMU guest 状态"
    if ! docker exec "$CONTAINER_NAME" test -S /tmp/qemu-normal.sock 2>/dev/null; then
        log_error "Unix socket /tmp/qemu-normal.sock 不存在。QEMU 是否已启动？"
        log_error "请先执行: ./qemu/start.sh"
        exit 1
    fi
    # 发送空回车测试 guest 是否响应
    local reply
    reply=$(docker exec "$CONTAINER_NAME" bash -c \
        "(sleep 1; printf '\n'; sleep 2) | socat - UNIX:/tmp/qemu-normal.sock 2>/dev/null" || echo "")
    if echo "$reply" | grep -q "#"; then
        log_info "QEMU guest 响应 ✓"
    else
        log_warn "QEMU guest 未响应（可能还在启动中，约需 90s）"
        log_warn "如果 QEMU 刚启动，请等待后重试"
    fi
}

start_http_server() {
    log_step "启动 HTTP 服务器（容器内 10.0.2.2:$HTTP_PORT）"
    # 停止可能存在的旧 HTTP 服务器
    docker exec "$CONTAINER_NAME" bash -c \
        "pkill -f 'python3 -m http.server $HTTP_PORT' 2>/dev/null; true" || true
    sleep 1
    docker exec -d "$CONTAINER_NAME" bash -c \
        "cd $SHARED_DIR && python3 -m http.server $HTTP_PORT > /tmp/httpd-deploy.log 2>&1"
    sleep 2
    log_info "HTTP 服务器在 $SHARED_DIR 上监听 $HTTP_PORT"
}

stop_http_server() {
    docker exec "$CONTAINER_NAME" bash -c \
        "pkill -f 'python3 -m http.server $HTTP_PORT' 2>/dev/null; true" || true
}

deploy_ta() {
    log_step "部署 TA 到 /lib/optee_armtz/"
    docker exec "$CONTAINER_NAME" bash -c "
    (
        sleep 1
        printf '\n'
        sleep 1
        printf 'mkdir -p /lib/optee_armtz /data/kms\n'
        sleep 1
        printf 'wget -q -O /lib/optee_armtz/${TA_UUID}.ta http://10.0.2.2:${HTTP_PORT}/ta/${TA_UUID}.ta && echo TA_OK || echo TA_FAIL\n'
        sleep 10
        printf 'ls -lh /lib/optee_armtz/${TA_UUID}.ta\n'
        sleep 2
        printf 'killall tee-supplicant 2>/dev/null; sleep 1\n'
        sleep 2
        printf 'tee-supplicant -l /lib/optee_armtz &\n'
        sleep 3
        printf 'echo TEE_SUPPLICANT_OK\n'
        sleep 2
    ) | socat - UNIX:/tmp/qemu-normal.sock 2>&1
    "
    log_info "TA 已部署并重启 tee-supplicant"
}

deploy_ca() {
    log_step "部署 kms-api-server 并启动"
    docker exec "$CONTAINER_NAME" bash -c "
    (
        sleep 1
        printf '\n'
        sleep 1
        printf 'wget -q -O /tmp/kms-api-server http://10.0.2.2:${HTTP_PORT}/kms-api-server && chmod +x /tmp/kms-api-server && echo CA_OK || echo CA_FAIL\n'
        sleep 15
        printf 'killall kms-api-server 2>/dev/null; sleep 1; true\n'
        sleep 2
        printf 'KMS_DB_PATH=${KMS_DB_PATH} /tmp/kms-api-server > /tmp/kms.log 2>&1 &\n'
        sleep 5
        printf 'echo SERVER_STARTED\n'
        sleep 2
    ) | socat - UNIX:/tmp/qemu-normal.sock 2>&1
    "
    log_info "kms-api-server 已启动"
}

verify_deployment() {
    log_step "验证部署"
    sleep 3
    local max_attempts=10
    local attempt=0
    while [ $attempt -lt $max_attempts ]; do
        attempt=$((attempt + 1))
        if curl -sf http://localhost:3000/health >/dev/null 2>&1; then
            log_info "✓ KMS API 正常响应"
            curl -s http://localhost:3000/health | python3 -m json.tool 2>/dev/null || true
            return 0
        fi
        log_info "  等待 KMS 就绪... ($attempt/$max_attempts)"
        sleep 3
    done
    log_warn "KMS API 未就绪，查看日志："
    docker exec "$CONTAINER_NAME" bash -c "
        (sleep 1; printf 'tail -20 /tmp/kms.log\n'; sleep 3) | socat - UNIX:/tmp/qemu-normal.sock 2>/dev/null
    " || true
}

trap stop_http_server EXIT

case "${1:-all}" in
    all)
        check_qemu_running
        start_http_server
        deploy_ta
        deploy_ca
        stop_http_server
        verify_deployment
        log_info "部署完成 ✓  API: http://localhost:3000/health"
        ;;
    restart)
        check_qemu_running
        start_http_server
        deploy_ca
        stop_http_server
        verify_deployment
        ;;
    ta)
        check_qemu_running
        start_http_server
        deploy_ta
        stop_http_server
        log_info "TA 部署完成"
        ;;
    *)
        echo "用法: $0 [all|restart|ta]"
        exit 1 ;;
esac
