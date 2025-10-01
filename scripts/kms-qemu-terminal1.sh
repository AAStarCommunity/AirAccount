#!/bin/bash
# KMS QEMU - Terminal 1: QEMU启动器
# ⚠️ 重要：必须在Terminal 2和3都启动后再运行此脚本

echo "⚠️  请确保 Terminal 2 和 Terminal 3 已经启动并运行！"
echo "🚀 启动 QEMU (3秒后开始)..."
sleep 3

# 使用修复后的 SDK 脚本（包含 3000 端口转发）
docker exec -it teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=ON ./scripts/runtime/bin/start_qemuv8"