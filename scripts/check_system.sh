#!/bin/bash
# 系统环境检查脚本

set -e

echo "=== AirAccount 开发环境检查 ==="

# 检查操作系统
echo "1. 检查操作系统版本..."
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "✅ 检测到 macOS"
    sw_vers
    
    # 检查是否为支持的macOS版本 (12.0+)
    macos_version=$(sw_vers -productVersion | cut -d '.' -f 1)
    if [[ $macos_version -ge 12 ]]; then
        echo "✅ macOS版本支持 ($macos_version.x)"
    else
        echo "❌ macOS版本过低，建议升级到12.0+"
        exit 1
    fi
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "✅ 检测到 Linux"
    if command -v lsb_release &> /dev/null; then
        lsb_release -a
    else
        cat /etc/os-release
    fi
else
    echo "❌ 不支持的操作系统: $OSTYPE"
    echo "支持的系统: macOS 12+ 或 Ubuntu 20.04/22.04 LTS"
    exit 1
fi

# 检查硬件要求
echo "2. 检查硬件要求..."

# 检查内存
if [[ "$OSTYPE" == "darwin"* ]]; then
    memory_gb=$(sysctl -n hw.memsize | awk '{print int($1/1024/1024/1024)}')
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    memory_gb=$(grep MemTotal /proc/meminfo | awk '{print int($2/1024/1024)}')
fi

if [[ $memory_gb -ge 8 ]]; then
    echo "✅ 内存充足: ${memory_gb}GB (推荐8GB+)"
else
    echo "⚠️  内存可能不足: ${memory_gb}GB (推荐8GB+)"
fi

# 检查磁盘空间
echo "3. 检查磁盘空间..."
available_space=$(df -h . | tail -1 | awk '{print $4}')
echo "✅ 当前目录可用空间: $available_space"

echo "=== 系统检查完成 ==="