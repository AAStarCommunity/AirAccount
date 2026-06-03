#!/usr/bin/env bash
# qemu/stop.sh — 优雅停止 QEMU 和开发容器

set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/qemu/lib/log.sh"

CONTAINER_NAME="teaclave_dev_env"
SESSION="kms-qemu"

log_step "停止 KMS 和 QEMU"

# 1. 在 guest 内优雅停止 KMS
docker exec "$CONTAINER_NAME" bash -c \
    "echo 'pkill kms-api-server' | socat - TCP:localhost:54320" 2>/dev/null || true
sleep 1

# 2. 在 guest 内 poweroff
docker exec "$CONTAINER_NAME" bash -c \
    "echo 'poweroff' | socat - TCP:localhost:54320" 2>/dev/null || true
sleep 3

# 3. 强制杀 QEMU 进程
docker exec "$CONTAINER_NAME" bash -c "pkill -f qemu-system-aarch64 || true" 2>/dev/null || true

# 4. 杀掉端口监听
docker exec "$CONTAINER_NAME" bash -c \
    "kill -9 \$(lsof -ti:54320) 2>/dev/null; kill -9 \$(lsof -ti:54321) 2>/dev/null; true" 2>/dev/null || true

# 5. 关闭 tmux 会话
if tmux has-session -t "$SESSION" 2>/dev/null; then
    tmux kill-session -t "$SESSION"
    log_info "tmux 会话 $SESSION 已关闭"
fi

log_info "QEMU 已停止 ✓"
log_info "开发容器仍在运行（执行 'docker stop $CONTAINER_NAME' 完全停止）"
