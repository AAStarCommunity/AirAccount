#!/bin/bash
# KMS QEMU - Terminal 3: Secure World日志监听器
# 必须在Terminal 1 (QEMU) 之前启动

echo "📜 启动 Secure World 日志监听器 (端口 54321)..."
docker exec -it teaclave_dev_env bash -l -c "listen_on_secure_world_log"