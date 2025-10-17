#!/bin/bash
# Terminal 3: Secure World Log
# 这个脚本会监听端口54321,显示OP-TEE日志

echo "🔒 Starting Secure World Log listener..."

# 检查并杀掉占用 54321 端口的进程
echo "    检查端口 54321..."
if docker exec teaclave_dev_env lsof -ti:54321 2>/dev/null; then
    echo "    ⚠️  端口 54321 已被占用，正在释放..."
    docker exec teaclave_dev_env bash -c "kill -9 \$(lsof -ti:54321) 2>/dev/null || true"
    sleep 1
fi

# 或者使用 pkill 方式（更可靠）
docker exec teaclave_dev_env pkill -f "listen_on_secure_world_log" 2>/dev/null || true
docker exec teaclave_dev_env pkill -f "TCP-LISTEN:54321" 2>/dev/null || true
sleep 1

echo "    ✅ 端口 54321 已释放"
echo "    Listening on port 54321"
echo "    Waiting for QEMU to connect..."
echo ""

docker exec -it teaclave_dev_env bash -l -c "listen_on_secure_world_log"