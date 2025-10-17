#!/bin/bash
# Terminal 3: OP-TEE TA (Secure World) 监控
# 显示 TA 在 Secure World 的执行日志

echo "🔒 Terminal 3: OP-TEE TA (Secure World) 监控"
echo "=================================================="
echo ""
echo "功能："
echo "  - 监控 TA 执行日志"
echo "  - Secure World 操作"
echo "  - TA 命令处理"
echo ""
echo "TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 连接到 Docker 并监控 OP-TEE 日志
docker exec -it teaclave_dev_env bash -c "
echo '📝 OP-TEE Secure World 日志:'
echo '=================================================='
echo ''
echo '提示: OP-TEE 日志通常在 QEMU serial console 输出'
echo '      或者 Guest VM 的 dmesg 中'
echo ''

# 尝试读取 OP-TEE 相关的内核日志
# 通过 serial port 发送命令到 Guest VM
(
echo 'dmesg | grep -i optee | tail -50'
sleep 2
echo ''
echo '持续监控 dmesg (Ctrl+C 停止):'
echo '=================================================='
sleep 1
# 每2秒刷新一次 dmesg 中的 OP-TEE 日志
while true; do
    echo 'clear'
    sleep 0.5
    echo 'dmesg | grep -i -E \"(optee|teec|tee)\" | tail -30'
    sleep 0.5
    echo 'echo \"\"'
    sleep 0.5
    echo 'echo \"--- Refreshing in 3s (Ctrl+C to stop) ---\"'
    sleep 0.5
    echo 'sleep 3'
    sleep 2
done
) | socat - TCP:localhost:54320
"