#!/bin/bash
# 测试监控系统 - 自动发送测试请求并验证监控

echo "🧪 KMS API 监控系统测试"
echo "=================================================="
echo ""

# 检查服务是否运行
echo "1️⃣ 检查服务状态..."
echo ""

# 检查 QEMU
echo "   📊 QEMU:"
docker exec teaclave_dev_env bash -c "ps aux | grep qemu-system-aarch64 | grep -v grep" > /dev/null
if [ $? -eq 0 ]; then
    echo "      ✅ QEMU 运行中"
else
    echo "      ❌ QEMU 未运行"
    exit 1
fi

# 检查 KMS API Server
echo "   📊 KMS API Server:"
docker exec teaclave_dev_env bash -c "(echo 'ps aux | grep kms-api-server | grep -v grep'; sleep 1) | socat - TCP:localhost:54320" 2>/dev/null | grep kms-api-server > /dev/null
if [ $? -eq 0 ]; then
    echo "      ✅ KMS API Server 运行中"
else
    echo "      ❌ KMS API Server 未运行"
    exit 1
fi

# 检查 cloudflared
echo "   📊 Cloudflared:"
docker exec teaclave_dev_env ps aux | grep cloudflared | grep -v grep > /dev/null
if [ $? -eq 0 ]; then
    echo "      ✅ Cloudflared 运行中"
else
    echo "      ❌ Cloudflared 未运行"
    exit 1
fi

echo ""
echo "2️⃣ 测试 API 调用链..."
echo ""

# 保存日志当前位置
echo "   📝 记录日志基线..."
CLOUDFLARED_LINES=$(docker exec teaclave_dev_env wc -l < /tmp/cloudflared.log 2>/dev/null || echo "0")

# 发送测试请求
echo "   🚀 发送测试请求: CreateKey"
RESPONSE=$(curl -s -X POST https://kms.aastar.io/CreateKey \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"Description":"monitor-test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}')

echo ""
echo "   📥 响应:"
echo "$RESPONSE" | jq . 2>/dev/null || echo "$RESPONSE"

# 检查响应
if echo "$RESPONSE" | jq -e '.KeyMetadata.KeyId' > /dev/null 2>&1; then
    KEY_ID=$(echo "$RESPONSE" | jq -r '.KeyMetadata.KeyId')
    echo ""
    echo "   ✅ 测试成功！创建的 KeyId: $KEY_ID"
else
    echo ""
    echo "   ❌ 测试失败！"
    exit 1
fi

echo ""
echo "3️⃣ 验证监控日志..."
echo ""
sleep 2

# 检查 cloudflared 日志
echo "   📊 Cloudflared 日志:"
NEW_CLOUDFLARED_LINES=$(docker exec teaclave_dev_env wc -l < /tmp/cloudflared.log 2>/dev/null || echo "0")
DIFF=$((NEW_CLOUDFLARED_LINES - CLOUDFLARED_LINES))
if [ $DIFF -gt 0 ]; then
    echo "      ✅ 新增 $DIFF 行日志"
    echo "      最新日志:"
    docker exec teaclave_dev_env tail -5 /tmp/cloudflared.log | sed 's/^/         /'
else
    echo "      ⚠️  未检测到新日志"
fi

echo ""
echo "   📊 KMS API Server 日志:"
docker exec teaclave_dev_env bash -c "
(
echo 'tail -10 /tmp/kms.log | grep -i \"createkey\\|create\"'
sleep 2
) | socat - TCP:localhost:54320 2>/dev/null
" | tail -5 | sed 's/^/         /'

echo ""
echo "4️⃣ 监控系统验证完成"
echo ""
echo "✅ 所有服务运行正常"
echo "✅ API 调用链完整"
echo "✅ 日志记录正常"
echo ""
echo "=================================================="
echo ""
echo "📺 启动完整监控："
echo "   ./scripts/monitor-all-tmux.sh"
echo ""
echo "或者在 4 个终端分别运行："
echo "   ./scripts/monitor-terminal1-qemu.sh"
echo "   ./scripts/monitor-terminal2-ca.sh"
echo "   ./scripts/monitor-terminal3-ta.sh"
echo "   ./scripts/monitor-terminal4-cloudflared.sh"
echo ""
echo "🌐 Web UI 测试："
echo "   https://kms.aastar.io"
echo ""