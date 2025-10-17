#!/bin/bash
# Terminal 2: KMS API Server (CA) 监控 - 直接读取共享日志
# 真实的 CA 日志，不依赖 socat

echo "🔐 Terminal 2: KMS API Server (CA) 监控 (真实日志)"
echo "=================================================="
echo ""
echo "功能："
echo "  - 显示真实的 CA Rust 日志"
echo "  - HTTP 请求处理详情"
echo "  - CA → TA 调用链"
echo ""
echo "日志来源: /root/shared/kms-api.log (QEMU Guest)"
echo "读取位置: /opt/teaclave/shared/kms-api.log (Docker)"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 检查日志文件是否存在
if ! docker exec teaclave_dev_env test -f /opt/teaclave/shared/kms-api.log; then
    echo "❌ 日志文件不存在！"
    echo ""
    echo "请先运行："
    echo "  ./scripts/restart-kms-with-shared-log.sh"
    echo ""
    echo "或者手动在 QEMU Guest 中将日志重定向到共享目录："
    echo "  cd /root/shared"
    echo "  killall kms-api-server"
    echo "  ./kms-api-server > /root/shared/kms-api.log 2>&1 &"
    echo ""
    exit 1
fi

# 显示进程信息
echo "📊 KMS API Server 状态:"
SERVICE_STATUS=$(curl -s https://kms.aastar.io/health 2>&1)
if echo "$SERVICE_STATUS" | grep -q "healthy"; then
    echo "   ✅ 运行中"
    echo "   Status: $(echo "$SERVICE_STATUS" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)"
    echo "   TA Mode: $(echo "$SERVICE_STATUS" | grep -o '"ta_mode":"[^"]*"' | cut -d'"' -f4)"
else
    echo "   ⚠️  状态未知"
fi

echo ""
echo "📝 CA 实时日志:"
echo "=================================================="
echo ""

# 直接 tail 共享目录中的日志文件
docker exec -it teaclave_dev_env tail -f /opt/teaclave/shared/kms-api.log