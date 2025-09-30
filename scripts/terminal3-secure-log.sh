#!/bin/bash
# Terminal 3: Secure World Log
# 这个脚本会监听端口54321,显示OP-TEE日志

echo "🔒 Starting Secure World Log listener..."
echo "    Listening on port 54321"
echo "    Waiting for QEMU to connect..."
echo ""

docker exec -it teaclave_dev_env bash -l -c "listen_on_secure_world_log"