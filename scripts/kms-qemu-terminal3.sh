#!/bin/bash
# KMS QEMU - Terminal 3: Secure World日志监听器
# 必须在Terminal 1 (QEMU) 之前启动

echo "🔄 清理旧的 QEMU 和监听器进程..."
docker exec teaclave_dev_env pkill -f qemu-system-aarch64 || true
docker exec teaclave_dev_env pkill -f listen_on_guest_vm_shell || true
docker exec teaclave_dev_env pkill -f listen_on_secure_world_log || true
docker exec teaclave_dev_env pkill -f "socat.*54320" || true
docker exec teaclave_dev_env pkill -f "socat.*54321" || true
sleep 2
echo "✅ 清理完成"
echo ""

echo "📜 启动 Secure World 日志监听器 (端口 54321)..."
docker exec -it teaclave_dev_env bash -l -c "listen_on_secure_world_log"