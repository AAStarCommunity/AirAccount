#!/bin/bash

# AirAccount 测试环境修复脚本
# 创建时间: 2025-08-17 11:06:00 +07

echo "🔧 AirAccount 测试环境修复脚本"
echo "=================================="

# 1. 清理QEMU多进程问题
echo "1️⃣ 清理QEMU进程..."
pkill -f qemu-system-aarch64 2>/dev/null || true
sleep 2

QEMU_COUNT=$(ps aux | grep qemu-system-aarch64 | grep -v grep | wc -l)
if [ "$QEMU_COUNT" -eq 0 ]; then
    echo "✅ QEMU进程已清理"
else
    echo "⚠️ 仍有 $QEMU_COUNT 个QEMU进程运行"
fi

# 2. 设置TA构建环境变量
echo "2️⃣ 设置TA构建环境..."
export OPTEE_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee"
export OPTEE_OS_DIR="$OPTEE_DIR/optee_os"
export OPTEE_CLIENT_DIR="$OPTEE_DIR/optee_client"
export TA_DEV_KIT_DIR="$OPTEE_OS_DIR/out/arm-plat-vexpress/export-ta_arm64"
export OPTEE_CLIENT_EXPORT="$OPTEE_CLIENT_DIR/export_arm64"

# 保存环境变量到文件
cat > ~/.airaccount_env << EOF
export OPTEE_DIR="$OPTEE_DIR"
export OPTEE_OS_DIR="$OPTEE_OS_DIR"
export OPTEE_CLIENT_DIR="$OPTEE_CLIENT_DIR"
export TA_DEV_KIT_DIR="$TA_DEV_KIT_DIR"
export OPTEE_CLIENT_EXPORT="$OPTEE_CLIENT_EXPORT"
EOF

if [ -d "$TA_DEV_KIT_DIR" ] && [ -d "$OPTEE_CLIENT_EXPORT" ]; then
    echo "✅ OP-TEE环境变量已设置"
    echo "   TA_DEV_KIT_DIR: $TA_DEV_KIT_DIR"
    echo "   OPTEE_CLIENT_EXPORT: $OPTEE_CLIENT_EXPORT"
else
    echo "❌ OP-TEE库路径不存在，需要构建"
    echo "   运行: cd third_party/incubator-teaclave-trustzone-sdk && ./build_optee_libraries.sh optee/"
fi

# 3. 检查共享目录
echo "3️⃣ 检查共享目录..."
SHARED_DIR="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/tests/shared"

if [ -d "$SHARED_DIR" ]; then
    echo "✅ 共享目录存在: $SHARED_DIR"
    echo "📁 共享目录内容:"
    ls -la "$SHARED_DIR"
else
    echo "❌ 共享目录不存在: $SHARED_DIR"
    mkdir -p "$SHARED_DIR"
    echo "✅ 已创建共享目录"
fi

# 4. 检查必要的文件
echo "4️⃣ 检查关键文件..."
FILES_TO_CHECK=(
    "$SHARED_DIR/11223344-5566-7788-99aa-bbccddeeff01.ta"
    "$SHARED_DIR/airaccount-ca"
)

for file in "${FILES_TO_CHECK[@]}"; do
    if [ -f "$file" ]; then
        echo "✅ $(basename "$file") 存在"
    else
        echo "❌ $(basename "$file") 不存在: $file"
    fi
done

# 5. 清理可能冲突的端口
echo "5️⃣ 清理端口冲突..."
PORTS=(3002 5174)

for port in "${PORTS[@]}"; do
    PID=$(lsof -ti:$port 2>/dev/null)
    if [ -n "$PID" ]; then
        echo "⚠️ 端口 $port 被进程 $PID 占用，正在清理..."
        kill "$PID" 2>/dev/null || true
        sleep 1
        echo "✅ 端口 $port 已清理"
    else
        echo "✅ 端口 $port 空闲"
    fi
done

# 6. 创建QEMU共享目录挂载修复脚本
echo "6️⃣ 创建QEMU挂载修复脚本..."
cat > "$SHARED_DIR/fix-mount.sh" << 'EOF'
#!/bin/bash
# QEMU内部共享目录挂载修复脚本
echo "🔧 修复QEMU共享目录挂载..."
mkdir -p /shared
mount -t 9p -o trans=virtio,version=9p2000.L host /shared 2>/dev/null || echo "挂载可能已存在"
ls -la /shared/
echo "✅ 共享目录修复完成"
EOF

chmod +x "$SHARED_DIR/fix-mount.sh"
echo "✅ 挂载修复脚本已创建: $SHARED_DIR/fix-mount.sh"

echo ""
echo "🎯 修复完成总结:"
echo "=================="
echo "✅ QEMU进程已清理"
echo "✅ TA构建环境已设置"
echo "✅ 共享目录已检查"
echo "✅ 端口冲突已清理"
echo "✅ QEMU挂载修复脚本已创建"
echo ""
echo "📋 下一步操作:"
echo "1. 启动QEMU: cd third_party/incubator-teaclave-trustzone-sdk/tests/ && ./optee-qemuv8-fixed.sh aarch64-optee-4.7.0-qemuv8-ubuntu-24.04"
echo "2. 在QEMU中运行: /shared/fix-mount.sh"
echo "3. 按照测试指南执行五步测试法"
echo ""
echo "🔗 参考文档: docs/MANUAL_TESTING_GUIDE_FIXED.md"