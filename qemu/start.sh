#!/usr/bin/env bash
# qemu/start.sh — 启动 QEMU + OP-TEE 环境
#
# 默认使用 tmux 三窗格（推荐）：
#   窗格 1：QEMU 主控
#   窗格 2：Guest VM Shell（Linux Normal World）
#   窗格 3：Secure World 日志（OP-TEE TA 输出）
#
# 用法：
#   ./qemu/start.sh           # tmux 模式（推荐）
#   ./qemu/start.sh --no-tmux # 单终端模式（日志写文件）

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/qemu/lib/log.sh"

CONTAINER_NAME="teaclave_dev_env"
# OrbStack VirtioFS 无法将 /opt/teaclave/images 暴露给容器，
# setup.sh 通过 docker cp 将镜像复制到容器内 /opt/qemu-images/
IMG_DIR="/opt/qemu-images"
IMG_NAME="x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory"
SHARED_DIR="/opt/teaclave/shared"
SESSION="kms-qemu"

USE_TMUX=true
for arg in "$@"; do
    [ "$arg" = "--no-tmux" ] && USE_TMUX=false
done

check_container() {
    if ! docker ps --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        log_error "开发容器未运行！先执行: ./qemu/setup.sh"
        exit 1
    fi
}

check_binaries() {
    log_step "检查构建产物"
    if ! docker exec "$CONTAINER_NAME" test -f "$SHARED_DIR/kms-api-server"; then
        log_warn "kms-api-server 未找到，先执行构建..."
        "$REPO_ROOT/qemu/build.sh" all
    fi
    if ! docker exec "$CONTAINER_NAME" test -f "$SHARED_DIR/ta/4319f351-0b24-4097-b659-80ee4f824cdd.ta"; then
        log_warn "TA 未找到，先执行构建..."
        "$REPO_ROOT/qemu/build.sh" ta
    fi
    log_info "构建产物 OK"
}

kill_old_qemu() {
    docker exec "$CONTAINER_NAME" bash -c "pkill -f qemu-system-aarch64 || true" 2>/dev/null || true
    docker exec "$CONTAINER_NAME" bash -c "kill -9 \$(lsof -ti:54320) 2>/dev/null; true" 2>/dev/null || true
    docker exec "$CONTAINER_NAME" bash -c "kill -9 \$(lsof -ti:54321) 2>/dev/null; true" 2>/dev/null || true
    sleep 1
}

# QEMU 启动命令（含 KMS 3000 端口转发）
# 串口使用 Unix socket（比 TCP 更可靠，避免端口冲突）
# Normal World 串口: /tmp/qemu-normal.sock
# Secure World 日志: /tmp/qemu-secure.log
QEMU_CMD="cd $IMG_DIR/$IMG_NAME && ./qemu-system-aarch64 \
    -nodefaults \
    -nographic \
    -serial unix:/tmp/qemu-normal.sock,server,nowait \
    -serial file:/tmp/qemu-secure.log \
    -smp 2 \
    -s \
    -machine virt,secure=on,acpi=off,gic-version=3 \
    -cpu cortex-a57 \
    -d unimp -semihosting-config enable=on,target=native \
    -m 1057 \
    -bios bl1.bin \
    -initrd rootfs.cpio.gz \
    -append 'console=ttyAMA0,115200 keep_bootcon root=/dev/vda2' \
    -kernel Image \
    -fsdev local,id=fsdev0,path=${SHARED_DIR},security_model=none \
    -device virtio-9p-device,fsdev=fsdev0,mount_tag=host \
    -netdev 'user,id=vmnic,hostfwd=tcp::3000-:3000,hostfwd=tcp::54433-:4433' \
    -device virtio-net-device,netdev=vmnic"

start_tmux() {
    log_step "启动 tmux 会话: $SESSION"

    if tmux has-session -t "$SESSION" 2>/dev/null; then
        log_warn "会话 $SESSION 已存在，杀掉重建..."
        tmux kill-session -t "$SESSION"
    fi

    # 窗格布局：左=QEMU主控，右上=Guest Shell，右下=TA日志
    tmux new-session -d -s "$SESSION" -x 220 -y 55

    # 窗格 0：Secure World 日志（tail 文件，QEMU 将 TA 输出写入 /tmp/qemu-secure.log）
    tmux send-keys -t "$SESSION:0" \
        "echo 'Secure World Log' && docker exec -it $CONTAINER_NAME bash -c 'tail -f /tmp/qemu-secure.log 2>/dev/null || sleep 5 && tail -f /tmp/qemu-secure.log'" Enter

    # 竖向分割 → 窗格 1：Guest VM Shell（通过 Unix socket 连接）
    tmux split-window -t "$SESSION:0" -v
    tmux send-keys -t "$SESSION:0.1" \
        "echo 'Guest VM Shell (wait ~90s for boot)' && sleep 5 && docker exec -it $CONTAINER_NAME bash -c 'socat - UNIX:/tmp/qemu-normal.sock'" Enter

    # 横向分割 → 窗格 2：QEMU 主控
    tmux split-window -t "$SESSION:0.0" -h
    tmux send-keys -t "$SESSION:0.2" \
        "echo 'QEMU starting...' && sleep 2 && docker exec -it $CONTAINER_NAME bash -l -c \"$QEMU_CMD\"" Enter

    sleep 1
    tmux attach-session -t "$SESSION"
}

start_no_tmux() {
    log_step "启动 QEMU（单终端模式，日志写 /tmp/qemu-secure.log）"
    log_info "Guest VM Shell: docker exec $CONTAINER_NAME socat - UNIX:/tmp/qemu-normal.sock"
    log_info "Secure World 日志: docker exec $CONTAINER_NAME tail -f /tmp/qemu-secure.log"

    # 前台启动 QEMU（串口写 Unix socket + 文件）
    docker exec -it "$CONTAINER_NAME" bash -l -c "$QEMU_CMD"
}

check_container
check_binaries
kill_old_qemu

if $USE_TMUX; then
    if ! command -v tmux &>/dev/null; then
        log_warn "tmux 未安装，回退到单终端模式"
        start_no_tmux
    else
        start_tmux
    fi
else
    start_no_tmux
fi
