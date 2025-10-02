#!/bin/bash
# KMS Passkey - 监控 Guest VM Shell (端口 54330)

echo "🖥️  连接到 KMS Passkey Guest VM Shell (端口 54330)..."
docker exec -it kms_passkey_dev bash -c "socat - TCP:localhost:54320,crlf"
