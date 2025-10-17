#!/bin/bash
# KMS 一键启动 + 自动初始化测试钱包

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "🚀 启动 KMS..."
"$SCRIPT_DIR/kms-auto-start.sh"

echo ""
echo "⏳ 等待 API Server 启动完成..."
for i in {1..30}; do
    if curl -s http://localhost:3000/health > /dev/null 2>&1; then
        echo "✅ API Server 已就绪"
        break
    fi
    echo -n "."
    sleep 1
done

if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo ""
    echo "⚠️  API Server 未响应，请检查日志"
    exit 1
fi

echo ""
echo "🔄 初始化开发测试钱包..."
"$SCRIPT_DIR/kms-init-dev-wallets.sh"

echo ""
echo "========================================"
echo "✅ KMS 已启动，测试钱包已就绪！"
echo "========================================"
echo ""
echo "📊 快速测试:"
echo "  curl http://localhost:3000/health"
echo "  curl https://kms.aastar.io/health"
echo ""
echo "📋 查看钱包:"
echo "  curl -s -X POST http://localhost:3000/ListKeys \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -H 'x-amz-target: TrentService.ListKeys' \\"
echo "    -d '{}' | jq ."
echo ""
echo "🔍 监控日志:"
echo "  ./scripts/kms-qemu-terminal2-enhanced.sh  # CA 日志"
echo ""
