#!/bin/bash

echo "🚀 AirAccount 手动集成测试"
echo "========================"

cd third_party/incubator-teaclave-trustzone-sdk/tests

# 确保共享目录存在
mkdir -p shared

# 复制构建产物
echo "📁 复制构建产物到共享目录..."
cp ../../../packages/airaccount-ta-simple/target/aarch64-unknown-linux-gnu/release/11223344-5566-7788-99aa-bbccddeeff01.ta shared/
cp ../../../packages/airaccount-ca/target/aarch64-unknown-linux-gnu/debug/airaccount-ca shared/
chmod +x shared/airaccount-ca

echo "✅ 文件已准备:"
ls -la shared/

echo ""
echo "🖥️  启动QEMU OP-TEE环境..."
echo "请手动执行以下测试步骤："
echo ""
echo "1. 登录: 用户名 'root' (无密码)"
echo "2. 挂载共享文件夹:"
echo "   mkdir -p /shared && mount -t 9p -o trans=virtio host /shared"
echo "3. 安装TA文件:"
echo "   cp /shared/*.ta /lib/optee_armtz/"
echo "4. 运行基础测试:"
echo "   /shared/airaccount-ca hello"
echo "   /shared/airaccount-ca echo 'Hello AirAccount TEE!'"
echo "   /shared/airaccount-ca test"
echo "   /shared/airaccount-ca wallet"
echo "5. 退出QEMU: 按 Ctrl+A 然后 X"
echo ""
echo "启动QEMU中..."
echo ""

# 直接启动QEMU而不使用screen
cd aarch64-optee-4.7.0-qemuv8-ubuntu-24.04

# 使用系统qemu-system-aarch64
exec qemu-system-aarch64 \
    -nodefaults \
    -nographic \
    -serial stdio \
    -smp 2 \
    -machine virt,secure=on,acpi=off,gic-version=3 \
    -cpu cortex-a57 \
    -d unimp -semihosting-config enable=on,target=native \
    -m 1057 \
    -bios bl1.bin \
    -initrd rootfs.cpio.gz \
    -append 'console=ttyAMA0,115200 keep_bootcon root=/dev/vda2' \
    -kernel Image \
    -fsdev local,id=fsdev0,path=$(pwd)/../shared,security_model=none \
    -device virtio-9p-device,fsdev=fsdev0,mount_tag=host \
    -netdev user,id=vmnic,hostfwd=:127.0.0.1:54433-:4433 \
    -device virtio-net-device,netdev=vmnic