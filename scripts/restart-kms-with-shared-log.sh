#!/bin/bash
# 重启 KMS API Server，将日志输出到共享目录
# 这样可以在 Docker 中直接读取日志，无需 socat

echo "🔄 重启 KMS API Server (日志输出到共享目录)"
echo "=================================================="
echo ""

echo "1. 连接到 QEMU Guest VM..."
docker exec teaclave_dev_env bash -c "
timeout 10 bash -c '
(
echo \"\"
sleep 1
echo \"# 停止现有的 KMS API Server\"
echo \"killall kms-api-server 2>/dev/null\"
sleep 2

echo \"\"
echo \"# 启动 KMS API Server，日志写入共享目录\"
echo \"cd /root/shared && nohup ./kms-api-server > /root/shared/kms-api.log 2>&1 &\"
sleep 3

echo \"\"
echo \"# 验证进程\"
echo \"ps aux | grep kms-api-server | grep -v grep\"
sleep 2

echo \"\"
echo \"# 检查日志文件\"
echo \"ls -lh /root/shared/*.log\"
sleep 1
) | socat - TCP:localhost:54320
' 2>&1
" || echo "⚠️  命令执行完成（可能超时是正常的）"

echo ""
echo "2. 等待服务启动..."
sleep 3

echo ""
echo "3. 验证日志文件..."
if docker exec teaclave_dev_env test -f /opt/teaclave/shared/kms-api.log; then
    echo "✅ 日志文件已创建: /opt/teaclave/shared/kms-api.log"
    echo ""
    echo "最新日志内容："
    docker exec teaclave_dev_env tail -10 /opt/teaclave/shared/kms-api.log
else
    echo "❌ 日志文件未找到"
    echo ""
    echo "请手动在 QEMU Guest 中执行："
    echo "  1. 连接: socat - TCP:localhost:54320"
    echo "  2. 登录: root (无密码)"
    echo "  3. 运行: cd /root/shared && ./kms-api-server > /root/shared/kms-api.log 2>&1 &"
fi

echo ""
echo "4. 测试 API..."
RESPONSE=$(curl -s https://kms.aastar.io/health)
if echo "$RESPONSE" | grep -q "healthy"; then
    echo "✅ KMS API Server 运行正常"
else
    echo "❌ KMS API Server 可能未运行"
fi

echo ""
echo "=================================================="
echo "✅ 完成！"
echo ""
echo "现在可以在 Docker 中监控日志："
echo "  docker exec -it teaclave_dev_env tail -f /opt/teaclave/shared/kms-api.log"
echo ""
echo "或使用新的监控脚本："
echo "  ./scripts/monitor-terminal2-ca-direct.sh"
echo ""