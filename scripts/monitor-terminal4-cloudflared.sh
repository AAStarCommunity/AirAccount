#!/bin/bash
# Terminal 4: Cloudflared Tunnel 监控
# 显示公网流量和隧道状态

echo "🌐 Terminal 4: Cloudflared Tunnel 监控"
echo "=================================================="
echo ""
echo "功能："
echo "  - 监控隧道连接状态"
echo "  - 公网 → Docker 流量"
echo "  - 请求响应日志"
echo ""
echo "公网地址: https://kms.aastar.io"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 监控 cloudflared 日志
docker exec -it teaclave_dev_env bash -c "
echo '📊 Cloudflared 进程信息:'
ps aux | grep cloudflared | grep -v grep
echo ''
echo '📝 Cloudflared 隧道日志 (持续监控):'
echo '=================================================='
tail -f /tmp/cloudflared.log
"