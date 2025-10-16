#!/bin/bash
# KMS QEMU - Terminal 1: QEMU启动器 + 自动初始化测试钱包
# ⚠️ 重要：必须在Terminal 2和3都启动后再运行此脚本

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "⚠️  请确保 Terminal 2 和 Terminal 3 已经启动并运行！"
echo "🚀 启动 QEMU (3秒后开始)..."
sleep 3

# 使用修复后的 SDK 脚本（包含 3000 端口转发）
docker exec -it teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=ON ./scripts/runtime/bin/start_qemuv8" &

QEMU_PID=$!

echo ""
echo "⏳ 等待 API Server 启动..."
for i in {1..30}; do
    if curl -s http://localhost:3000/health > /dev/null 2>&1; then
        echo ""
        echo "✅ API Server 已就绪"
        break
    fi
    sleep 1
    echo -n "."
done

if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo ""
    echo "🔄 初始化开发测试钱包..."
    "$SCRIPT_DIR/kms-init-dev-wallets.sh"

    echo ""
    echo "========================================"
    echo "✅ QEMU 已启动，测试钱包已就绪！"
    echo "========================================"
    echo ""
    echo "按 Ctrl+C 退出 QEMU"
else
    echo ""
    echo "⚠️  API Server 未启动，跳过钱包初始化"
    echo "可以稍后手动运行: ./scripts/kms-init-dev-wallets.sh"
    echo ""
fi

# 等待 QEMU 进程
wait $QEMU_PID