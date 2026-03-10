#!/bin/bash
# Terminal 2: KMS API Server (CA) 监控
# 实时显示 CA 层的 HTTP 请求和 TA 调用

echo "🔐 Terminal 2: KMS API Server (CA) 监控"
echo "=================================================="
echo ""
echo "功能："
echo "  - 监控 HTTP API 请求"
echo "  - CA → TA 调用链"
echo "  - 请求处理日志"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 连接到 QEMU Guest VM 并监控 KMS API Server 日志
docker exec -it teaclave_dev_env bash -c "
# 发送命令到 QEMU Guest VM
(
echo ''
echo '📊 KMS API Server 进程信息:'
sleep 1
echo 'ps aux | grep kms-api-server | grep -v grep'
sleep 2
echo ''
echo '📝 KMS API Server 日志 (持续监控):'
echo '=================================================='
sleep 1
echo 'tail -f /tmp/kms.log'
sleep 1
) | socat - TCP:localhost:54320
"