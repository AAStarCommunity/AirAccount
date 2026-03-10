#!/bin/bash
# 在QEMU Guest VM中运行此脚本来部署KMS

echo "🚀 KMS QEMU部署脚本"
echo "================================"

# 1. 创建挂载点
echo ""
echo "1️⃣ 创建共享目录挂载点..."
mkdir -p /root/shared

# 2. 挂载共享目录
echo "2️⃣ 挂载9p virtio共享目录..."
if mount -t 9p -o trans=virtio host /root/shared; then
    echo "✅ 共享目录挂载成功"
else
    echo "❌ 挂载失败！请确保QEMU启动时配置了9p virtio共享"
    echo "   需要的QEMU参数: -fsdev local,id=fsdev0,path=/opt/teaclave/shared,security_model=none"
    echo "                   -device virtio-9p-device,fsdev=fsdev0,mount_tag=host"
    exit 1
fi

# 3. 验证共享目录内容
echo ""
echo "3️⃣ 检查共享目录内容..."
ls -lh /root/shared/

# 4. 复制TA到系统目录
echo ""
echo "4️⃣ 部署TA到 /lib/optee_armtz/..."
cp /root/shared/4319f351-0b24-4097-b659-80ee4f824cdd.ta /lib/optee_armtz/

if [ $? -eq 0 ]; then
    echo "✅ TA部署成功"
    ls -lh /lib/optee_armtz/4319f351-*.ta
else
    echo "❌ TA部署失败"
    exit 1
fi

# 5. 验证可执行文件
echo ""
echo "5️⃣ 验证KMS可执行文件..."
if [ -f /root/shared/kms ]; then
    echo "✅ kms CLI工具存在"
else
    echo "❌ kms CLI工具不存在"
    exit 1
fi

if [ -f /root/shared/kms-api-server ]; then
    echo "✅ kms-api-server存在"
else
    echo "❌ kms-api-server不存在"
    exit 1
fi

# 6. 测试KMS
echo ""
echo "6️⃣ 测试KMS功能..."
cd /root/shared

echo ""
echo "测试: ./kms create-wallet"
./kms create-wallet

echo ""
echo "================================"
echo "🎉 KMS部署完成！"
echo ""
echo "使用方法:"
echo "  cd /root/shared"
echo "  ./kms --help"
echo "  ./kms-api-server &"
echo ""
echo "或运行完整测试:"
echo "  bash /root/shared/test-kms-qemu.sh"