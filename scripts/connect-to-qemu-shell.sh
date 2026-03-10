#!/bin/bash
# 连接到运行中的QEMU Guest VM Shell

echo "🖥️  Connecting to QEMU Guest VM Shell..."
echo "    (via TCP serial port 54320)"
echo ""
echo "提示："
echo "  - 如果看到login提示，用户名: root，密码: (直接按回车)"
echo "  - 按 Ctrl+] 断开连接（不会停止QEMU）"
echo ""
echo "========================================"
echo ""

# 使用telnet连接到QEMU serial端口
docker exec -it teaclave_dev_env telnet localhost 54320