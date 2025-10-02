#!/bin/bash
# KMS Passkey - 监控 Secure World 日志 (端口 54331)

echo "🔒 连接到 KMS Passkey Secure World 日志 (端口 54331)..."
docker exec -it kms_passkey_dev bash -c "socat - TCP:localhost:54321"
