#!/bin/bash
# 启动 cloudflared 并启用 debug 日志以便监控

echo "🌐 启动 Cloudflared Tunnel (Debug 模式)"
echo "=================================================="
echo ""

# 停止现有的 cloudflared 进程
echo "1. 停止现有的 cloudflared 进程..."
docker exec teaclave_dev_env bash -c "pkill cloudflared" 2>/dev/null
sleep 2

# 清理僵尸进程
docker exec teaclave_dev_env bash -c "ps aux | grep 'cloudflared.*defunct' | awk '{print \$2}' | xargs -r kill -9" 2>/dev/null

echo "✅ 已停止旧进程"
echo ""

# 启动 cloudflared 并启用 debug 日志
echo "2. 启动 cloudflared (--loglevel debug)..."
docker exec -d teaclave_dev_env bash -c \
  "cloudflared tunnel --config /root/.cloudflared/config-docker.yml --loglevel debug run kms-tunnel > /tmp/cloudflared.log 2>&1"

sleep 3

# 验证进程状态
echo "3. 验证 cloudflared 状态..."
PROCESS_COUNT=$(docker exec teaclave_dev_env ps aux | grep cloudflared | grep -v grep | grep -v defunct | wc -l)

if [ "$PROCESS_COUNT" -gt 0 ]; then
    echo "✅ Cloudflared 运行中"
    docker exec teaclave_dev_env ps aux | grep cloudflared | grep -v grep | grep -v defunct | head -1
else
    echo "❌ Cloudflared 启动失败"
    exit 1
fi

echo ""
echo "4. 检查隧道连接..."
sleep 5

# 显示最新日志
docker exec teaclave_dev_env tail -10 /tmp/cloudflared.log | grep -E "Registered tunnel|error|Error"

echo ""
echo "=================================================="
echo "✅ Cloudflared 已启动 (Debug 模式)"
echo ""
echo "📊 监控日志："
echo "   docker exec teaclave_dev_env tail -f /tmp/cloudflared.log"
echo ""
echo "🧪 测试连接："
echo "   curl https://kms.aastar.io/health"
echo ""
echo "📺 启动完整监控："
echo "   ./scripts/monitor-all-tmux.sh"
echo ""