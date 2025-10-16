#!/bin/bash
# KMS QEMU - Terminal 3 v2: Secure World日志查看器
# 适配 kms-auto-start-v2.sh（监听器已启动，直接连接）

echo "📜 连接到 Secure World 日志监听器 (端口 54321)..."
echo "    (监听器已由 kms-auto-start-v2.sh 启动)"
echo "    Press Ctrl+C 退出"
echo ""

# 检查是否在交互式终端中
if [ -t 0 ]; then
    # 交互模式
    docker exec -it teaclave_dev_env socat - TCP:localhost:54321
else
    # 非交互模式（如后台运行）
    docker exec teaclave_dev_env socat - TCP:localhost:54321
fi
