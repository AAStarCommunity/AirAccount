#!/bin/bash
# Terminal 1: QEMU Control
# ⚠️ 注意: 必须在Terminal 2和3启动后再运行此脚本!

echo "🚀 Starting QEMU..."
echo ""
echo "⚠️  请确认:"
echo "    1. Terminal 2 已启动并显示 'Listening on TCP port 54320'"
echo "    2. Terminal 3 已启动并显示 'Listening on TCP port 54321'"
echo ""
echo "按回车继续,或Ctrl+C取消..."
read

docker exec -it teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8"