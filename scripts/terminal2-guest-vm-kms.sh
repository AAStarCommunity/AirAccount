#!/bin/bash
# Terminal 2: Guest VM Shell (with KMS port forwarding)
# 连接到已运行的QEMU（带3000端口转发）

echo "🖥️  Connecting to QEMU Guest VM..."
echo "    (QEMU should already be running with KMS port forwarding)"
echo ""

# 直接连接到已运行的QEMU的stdio
docker exec -it teaclave_dev_env bash -c "
    echo '正在连接到QEMU Guest VM...'
    echo '提示: 如果看到login提示，请使用:'
    echo '  用户名: root'
    echo '  密码: (直接按回车)'
    echo ''
    # 由于QEMU使用stdio，我们需要attach到正在运行的进程
    # 但这不太可行，所以我们提供命令让用户在terminal1中执行
    echo '由于QEMU使用 -serial stdio，请在terminal1的QEMU shell中执行以下命令:'
    echo ''
    echo '  mount -t 9p -o trans=virtio host /root/shared'
    echo '  cp /root/shared/*.ta /lib/optee_armtz/'
    echo '  cd /root/shared'
    echo '  ./kms-api-server'
    echo ''
    echo '或者，打开新终端执行:'
    echo '  docker exec -it teaclave_dev_env bash'
    echo ''
    read -p '按回车继续...'
"