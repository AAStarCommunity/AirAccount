#!/bin/bash
# Terminal 2: CA (Client Application) 监控 - 增强版
# 使用新的 3000 端口配置
# 显示 KMS API Server 的日志和状态

echo "🔐 Terminal 2: CA (Client Application) 监控"
echo "=================================================="
echo ""
echo "功能："
echo "  - 监控 KMS API Server (端口 3000)"
echo "  - HTTP 请求和响应"
echo "  - CA → TA 调用链"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 等待 QEMU 完全启动
sleep 3

# 检查 QEMU 是否在运行
if ! docker exec teaclave_dev_env pgrep -f qemu-system-aarch64 > /dev/null; then
    echo "❌ QEMU 未运行！"
    echo ""
    echo "请先启动 QEMU:"
    echo "  ./scripts/start-qemu-with-kms-port.sh"
    echo ""
    exit 1
fi

echo "✅ QEMU 运行中"
echo ""

# 检查端口转发是否包含 3000
QEMU_CMD=$(docker exec teaclave_dev_env ps aux | grep qemu-system-aarch64 | grep -v grep)
if echo "$QEMU_CMD" | grep -q "3000"; then
    echo "✅ QEMU 已配置 3000 端口转发"
else
    echo "⚠️  QEMU 可能未配置 3000 端口转发"
    echo ""
    echo "如果 API 无法访问，请重启 QEMU:"
    echo "  ./scripts/start-qemu-with-kms-port.sh"
    echo ""
fi

echo ""
echo "📊 检查 KMS API Server 状态..."
echo ""

# 尝试从 Docker 内访问
if timeout 3 docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health > /dev/null 2>&1; then
    echo "✅ KMS API Server 运行中 (http://127.0.0.1:3000)"

    # 显示健康信息
    HEALTH=$(docker exec teaclave_dev_env curl -s http://127.0.0.1:3000/health)
    echo ""
    echo "服务信息:"
    echo "$HEALTH" | grep -o '"status":"[^"]*"' | cut -d'"' -f4 | sed 's/^/  Status: /'
    echo "$HEALTH" | grep -o '"ta_mode":"[^"]*"' | cut -d'"' -f4 | sed 's/^/  TA Mode: /'
    echo "$HEALTH" | grep -o '"version":"[^"]*"' | cut -d'"' -f4 | sed 's/^/  Version: /'

else
    echo "⚠️  KMS API Server 未运行"
    echo ""
    echo "启动方法:"
    echo "  1. 在新终端运行: socat - TCP:localhost:54320"
    echo "  2. 在 QEMU 中执行:"
    echo "     cd /root/shared"
    echo "     ./kms-api-server > kms-api.log 2>&1 &"
    echo "  3. 按 Ctrl+C 退出 socat"
    echo ""
fi

echo ""
echo "=================================================="
echo "开始实时监控 CA 日志..."
echo "=================================================="
echo ""

# 方案 1: 如果日志写入共享目录，直接 tail
if docker exec teaclave_dev_env test -f /opt/teaclave/shared/kms-api.log; then
    echo "📝 监控共享目录日志: /opt/teaclave/shared/kms-api.log"
    echo ""
    docker exec -it teaclave_dev_env tail -f /opt/teaclave/shared/kms-api.log

# 方案 2: 尝试通过 socat 读取 QEMU 内的日志
else
    echo "📝 尝试通过 socat 读取 QEMU 内日志..."
    echo ""
    echo "提示: 如果没有日志输出，需要在 QEMU 中将日志重定向到共享目录:"
    echo "  cd /root/shared && ./kms-api-server > kms-api.log 2>&1 &"
    echo ""
    echo "或者使用实时 API 调用监控（从 Cloudflared）:"
    echo "  ./scripts/monitor-terminal2-ca-alt.sh"
    echo ""

    # 尝试通过 socat 查看日志
    docker exec -it teaclave_dev_env bash -c '
    timeout 3 bash -c '"'"'
    (
    echo "tail -f /tmp/kms.log 2>/dev/null || echo '\''日志文件不存在'\''"
    sleep 1
    ) | socat - TCP:localhost:54320 2>&1
    '"'"' || echo "⚠️  无法连接到 QEMU 串口，建议使用共享目录日志方式"
    '

    echo ""
    echo "=================================================="
    echo "💡 推荐使用共享目录日志方式，更稳定"
    echo "=================================================="
fi
