#!/bin/bash
# 安装基础开发工具脚本

set -e

echo "=== AirAccount 开发工具安装 ==="

# 检查操作系统
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "1. macOS环境 - 检查Homebrew..."
    
    # 检查是否已安装Homebrew
    if ! command -v brew &> /dev/null; then
        echo "⚠️  Homebrew未安装，正在安装..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        
        # 添加Homebrew到PATH (Apple Silicon Mac)
        if [[ $(uname -m) == "arm64" ]]; then
            echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
            eval "$(/opt/homebrew/bin/brew shellenv)"
        fi
    else
        echo "✅ Homebrew已安装: $(brew --version | head -1)"
    fi
    
    echo "2. 安装开发依赖包..."
    
    # 定义必需的包列表
    packages=(
        "automake"
        "coreutils" 
        "curl"
        "gmp"
        "gnutls"
        "libtool"
        "libusb"
        "make"
        "wget"
        "qemu"
        "git"
        "python3"
        "pkg-config"
    )
    
    # 检查并安装每个包
    for package in "${packages[@]}"; do
        if brew list "$package" &> /dev/null; then
            echo "✅ $package 已安装"
        else
            echo "📦 正在安装 $package..."
            brew install "$package"
        fi
    done
    
    echo "3. 验证关键工具..."
    
    # 验证安装的工具
    tools_to_check=(
        "qemu-system-aarch64 --version"
        "make --version"
        "git --version"
        "curl --version"
        "python3 --version"
    )
    
    for tool_check in "${tools_to_check[@]}"; do
        if eval "$tool_check" &> /dev/null; then
            tool_name=$(echo "$tool_check" | cut -d' ' -f1)
            version=$(eval "$tool_check" 2>&1 | head -1)
            echo "✅ $tool_name: $version"
        else
            tool_name=$(echo "$tool_check" | cut -d' ' -f1)
            echo "❌ $tool_name 验证失败"
            exit 1
        fi
    done
    
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "1. Linux环境 - 使用apt安装..."
    
    # 更新系统
    echo "更新系统包列表..."
    sudo apt update
    
    # 安装开发工具
    echo "2. 安装开发依赖包..."
    sudo apt install -y \
        build-essential \
        git \
        curl \
        python3 \
        python3-pip \
        uuid-dev \
        libssl-dev \
        libffi-dev \
        libglib2.0-dev \
        libpixman-1-dev \
        ninja-build \
        pkg-config \
        gcc-multilib \
        qemu-system-arm \
        qemu-user-static \
        make \
        wget
        
    echo "3. 验证Linux工具安装..."
    tools_to_check=(
        "qemu-system-aarch64 --version"
        "make --version" 
        "git --version"
        "curl --version"
        "python3 --version"
        "gcc --version"
    )
    
    for tool_check in "${tools_to_check[@]}"; do
        if eval "$tool_check" &> /dev/null; then
            tool_name=$(echo "$tool_check" | cut -d' ' -f1)
            version=$(eval "$tool_check" 2>&1 | head -1)
            echo "✅ $tool_name: $version"
        else
            tool_name=$(echo "$tool_check" | cut -d' ' -f1)
            echo "❌ $tool_name 验证失败"
            exit 1
        fi
    done
    
else
    echo "❌ 不支持的操作系统: $OSTYPE"
    echo "支持的系统: macOS 或 Linux"
    exit 1
fi

echo "=== 开发工具安装完成 ==="
echo "📝 提示: 可能需要重新打开终端以确保所有环境变量生效"