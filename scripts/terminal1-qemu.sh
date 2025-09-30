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

docker exec -it teaclave_dev_env bash -l -c "LISTEN_MODE=ON start_qemuv8"