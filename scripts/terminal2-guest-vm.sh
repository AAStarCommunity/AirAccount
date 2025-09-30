#!/bin/bash
# Terminal 2: Guest VM Shell
# 这个脚本会监听端口54320,等待QEMU连接

echo "🖥️  Starting Guest VM Shell listener..."
echo "    Listening on port 54320"
echo "    Waiting for QEMU to connect..."
echo ""

docker exec -it teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"