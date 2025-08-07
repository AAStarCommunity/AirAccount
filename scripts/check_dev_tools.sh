#!/bin/bash
# 检查开发工具安装状态脚本

set -e

echo "=== AirAccount 开发工具检查 ==="

# 检查操作系统
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "1. macOS环境 - 检查已安装的工具..."
    
    # 检查Homebrew
    if command -v brew &> /dev/null; then
        echo "✅ Homebrew: $(brew --version | head -1)"
    else
        echo "❌ Homebrew未安装"
        echo "请运行以下命令安装Homebrew："
        echo '/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"'
        exit 1
    fi
    
    echo "2. 检查关键开发工具..."
    
    # 检查各个工具
    missing_tools=()
    
    # 检查automake
    if automake --version &> /dev/null; then
        echo "✅ automake: $(automake --version | head -1)"
    else
        echo "❌ automake: 未安装"
        missing_tools+=("automake")
    fi
    
    # 检查coreutils (通过gls命令)
    if gls --version &> /dev/null; then
        echo "✅ coreutils: $(gls --version | head -1)"
    else
        echo "❌ coreutils: 未安装"
        missing_tools+=("coreutils")
    fi
    
    # 检查curl
    if curl --version &> /dev/null; then
        echo "✅ curl: $(curl --version | head -1)"
    else
        echo "❌ curl: 未安装"
        missing_tools+=("curl")
    fi
    
    # 检查make (系统自带或brew)
    if make --version &> /dev/null; then
        echo "✅ make: $(make --version | head -1)"
    elif brew list make &> /dev/null; then
        echo "✅ make: 已通过brew安装"
    else
        echo "❌ make: 未安装"
        missing_tools+=("make")
    fi
    
    # 检查brew包
    brew_packages=("gmp" "gnutls" "libtool" "libusb" "wget" "qemu")
    for package in "${brew_packages[@]}"; do
        if brew list "$package" &> /dev/null; then
            echo "✅ $package: 已通过brew安装"
        else
            echo "❌ $package: 未安装"
            missing_tools+=("$package")
        fi
    done
    
    # 检查git
    if git --version &> /dev/null; then
        echo "✅ git: $(git --version)"
    else
        echo "❌ git: 未安装"
        missing_tools+=("git")
    fi
    
    # 检查python3
    if python3 --version &> /dev/null; then
        echo "✅ python3: $(python3 --version)"
    else
        echo "❌ python3: 未安装"
        missing_tools+=("python3")
    fi
    
    # 检查pkg-config
    if pkg-config --version &> /dev/null; then
        echo "✅ pkg-config: $(pkg-config --version)"
    else
        echo "❌ pkg-config: 未安装"
        missing_tools+=("pkg-config")
    fi
    
    if [ ${#missing_tools[@]} -eq 0 ]; then
        echo ""
        echo "🎉 所有必需的开发工具都已安装！"
    else
        echo ""
        echo "⚠️  以下工具需要安装:"
        for tool in "${missing_tools[@]}"; do
            echo "   - $tool"
        done
        echo ""
        echo "可以运行以下命令安装缺失的工具:"
        echo "brew install ${missing_tools[*]}"
    fi
    
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    echo "1. Linux环境 - 检查系统工具..."
    
    # Linux工具检查
    tools_to_check=(
        "qemu-system-aarch64 --version"
        "make --version"
        "git --version" 
        "curl --version"
        "python3 --version"
        "gcc --version"
        "pkg-config --version"
    )
    
    missing_tools=()
    
    for tool_check in "${tools_to_check[@]}"; do
        tool_name=$(echo "$tool_check" | cut -d' ' -f1)
        if eval "$tool_check" &> /dev/null; then
            version=$(eval "$tool_check" 2>&1 | head -1)
            echo "✅ $tool_name: $version"
        else
            echo "❌ $tool_name: 未安装"
            missing_tools+=("$tool_name")
        fi
    done
    
    if [ ${#missing_tools[@]} -eq 0 ]; then
        echo ""
        echo "🎉 所有必需的开发工具都已安装！"
    else
        echo ""
        echo "⚠️  请先安装缺失的工具后再继续"
    fi
    
else
    echo "❌ 不支持的操作系统: $OSTYPE"
    exit 1
fi

echo "=== 开发工具检查完成 ==="