#!/bin/bash
# KMS 系统状态检查

echo "📊 KMS 系统状态"
echo "==============="
echo ""

echo "🐳 Docker 容器:"
docker ps --filter name=teaclave_dev_env --format "{{.Status}}" || echo "❌ 未运行"
echo ""

echo "🖥️  QEMU 进程:"
docker exec teaclave_dev_env ps aux | grep qemu-system-aarch64 | grep -v grep | wc -l | xargs -I {} echo "{} 个进程" || echo "❌ 无法检查"
echo ""

echo "🔌 端口转发:"
docker exec teaclave_dev_env ps aux | grep qemu | grep -o "hostfwd=[^[:space:]]*" || echo "未配置"
echo ""

echo "🌐 API Server (Docker 内):"
docker exec teaclave_dev_env curl -s -m 2 http://127.0.0.1:3000/health > /dev/null 2>&1 && echo "✅ 运行中" || echo "❌ 未运行"
echo ""

echo "🌍 API Server (Mac 本地):"
curl -s -m 2 http://localhost:3000/health > /dev/null 2>&1 && echo "✅ 可访问" || echo "❌ 不可访问"
echo ""

echo "☁️  API Server (公网):"
curl -s -m 2 https://kms.aastar.io/health > /dev/null 2>&1 && echo "✅ 可访问" || echo "❌ 不可访问"
echo ""

echo "🔐 cloudflared:"
ps aux | grep cloudflared | grep -v grep | wc -l | xargs -I {} echo "{} 个进程"
echo ""

echo "📝 QEMU 内 API Server 状态:"
docker exec teaclave_dev_env bash -c "(echo 'ps aux | grep kms-api-server | grep -v grep'; sleep 1) | timeout 3 socat - TCP:localhost:54320 2>/dev/null" || echo "❌ 无法检查 (QEMU 可能未运行)"
