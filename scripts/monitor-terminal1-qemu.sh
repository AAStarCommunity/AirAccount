#!/bin/bash
# Terminal 1: QEMU Guest VM 监控
# 显示 Guest VM 的 QEMU 输出和系统日志

echo "🖥️  Terminal 1: QEMU Guest VM 监控"
echo "=================================================="
echo ""
echo "功能："
echo "  - 监控 QEMU 启动日志"
echo "  - 查看 Guest VM 系统消息"
echo "  - 显示内核日志"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 连接到 Docker 并监控 QEMU 日志
docker exec -it teaclave_dev_env bash -c "
echo '📊 QEMU 进程信息:'
ps aux | grep qemu-system-aarch64 | grep -v grep
echo ''
echo '📝 最近的 QEMU 日志 (持续监控):'
echo '=================================================='
tail -f /tmp/qemu.log
"