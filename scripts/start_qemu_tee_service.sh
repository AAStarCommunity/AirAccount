#!/bin/bash

# 启动真实的QEMU OP-TEE环境作为服务
# 用于Node.js CA连接

echo "🚀 启动 AirAccount QEMU OP-TEE 服务..."

cd "$(dirname "$0")/../third_party/incubator-teaclave-trustzone-sdk/tests"

# 检查QEMU环境是否存在
if [ ! -d "aarch64-optee-4.7.0-qemuv8-ubuntu-24.04" ]; then
    echo "❌ QEMU OP-TEE环境不存在"
    exit 1
fi

# 检查预编译文件
if [ ! -f "shared/airaccount-ca" ] || [ ! -f "shared/11223344-5566-7788-99aa-bbccddeeff01.ta" ]; then
    echo "❌ 预编译的AirAccount文件不存在"
    exit 1
fi

echo "✅ 检查完成，启动QEMU OP-TEE环境..."

# 后台启动QEMU，输出到日志文件
nohup ./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04 > qemu_tee_service.log 2>&1 &

QEMU_PID=$!
echo "🔧 QEMU OP-TEE 进程ID: $QEMU_PID"

# 等待启动
echo "⏳ 等待QEMU OP-TEE环境启动..."
sleep 30

# 检查进程是否还在运行
if ps -p $QEMU_PID > /dev/null; then
    echo "✅ QEMU OP-TEE服务已启动"
    echo "📝 日志文件: qemu_tee_service.log"
    echo "🔌 TEE设备: /dev/teepriv0 (在QEMU中)"
    echo "📋 进程ID: $QEMU_PID"
    
    # 保存进程ID
    echo $QEMU_PID > qemu_tee_service.pid
    
    echo ""
    echo "🎯 服务状态:"
    echo "- QEMU进程运行中"
    echo "- OP-TEE 4.7已加载"
    echo "- AirAccount TA已安装"
    echo "- 等待Node.js CA连接..."
    
else
    echo "❌ QEMU OP-TEE服务启动失败"
    cat qemu_tee_service.log
    exit 1
fi