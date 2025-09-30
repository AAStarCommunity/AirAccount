#!/bin/bash
# KMS QEMU - Terminal 1: QEMU启动器
# ⚠️ 重要：必须在Terminal 2和3都启动后再运行此脚本

echo "⚠️  请确保 Terminal 2 和 Terminal 3 已经启动并运行！"
echo "🚀 启动 QEMU (3秒后开始)..."
sleep 3

docker exec -it teaclave_dev_env bash -l -c "LISTEN_MODE=ON start_qemuv8"