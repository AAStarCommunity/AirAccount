#!/bin/bash
# KMS QEMU - Terminal 2: Guest VM监听器
# 必须在Terminal 1 (QEMU) 之前启动

echo "🔌 启动 Guest VM 监听器 (端口 54320)..."
docker exec -it teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"
