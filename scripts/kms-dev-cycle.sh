#!/bin/bash
# KMS 快速开发循环

set -e

echo "🔨 1. 编译部署..."
./scripts/kms-deploy.sh

echo ""
echo "🔄 2. 重启 QEMU 中的 API Server..."
docker exec teaclave_dev_env bash -c "(
echo 'pkill kms-api-server || true'
sleep 2
echo 'cd /root/shared && ./kms-api-server > kms-api.log 2>&1 &'
sleep 3
echo 'ps aux | grep kms-api-server | grep -v grep'
) | socat - TCP:localhost:54320" || {
    echo "⚠️  警告: 无法通过 socat 重启，QEMU 可能未运行"
    echo "请手动执行: ./scripts/terminal2-guest-vm.sh"
}

echo ""
echo "⏳ 等待服务启动..."
sleep 3

echo ""
echo "✅ 3. 测试 API..."
curl -s https://kms.aastar.io/health | jq . || echo "❌ API 测试失败"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📝 查看日志: ./scripts/terminal2-guest-vm.sh"
echo "   在 QEMU 内执行: tail -f /root/shared/kms-api.log"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
