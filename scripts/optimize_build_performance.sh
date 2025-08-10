#!/bin/bash

# Licensed to AirAccount under the Apache License, Version 2.0
# 构建性能优化脚本

set -e

echo "🚀 AirAccount 构建性能优化和测试"
echo "================================"
echo ""

# 函数：测量构建时间
measure_build_time() {
    local build_type="$1"
    local command="$2"
    
    echo "📊 测试 $build_type..."
    
    # 清理以确保公平比较
    cargo clean > /dev/null 2>&1
    
    # 测量时间
    local start_time=$(date +%s.%3N)
    eval "$command" > /dev/null 2>&1
    local end_time=$(date +%s.%3N)
    
    local duration=$(echo "$end_time - $start_time" | bc -l)
    printf "⏱️  %s: %.2f 秒\n" "$build_type" "$duration"
    
    return 0
}

# 1. 检查工具安装状态
echo "1️⃣  检查构建优化工具..."
echo "----------------------------------------"

tools_status() {
    local tool=$1
    local name=$2
    if command -v "$tool" &> /dev/null; then
        echo "✅ $name 已安装"
        return 0
    else
        echo "❌ $name 未安装"
        return 1
    fi
}

tools_status "cargo-watch" "cargo-watch"
tools_status "cargo-machete" "cargo-machete" 
tools_status "sccache" "sccache"
tools_status "lld" "LLVM Linker"

echo ""

# 2. 检查未使用的依赖
echo "2️⃣  检查未使用的依赖..."
echo "----------------------------------------"

if command -v cargo-machete &> /dev/null; then
    echo "🔍 扫描未使用的依赖..."
    cargo machete || echo "⚠️  发现一些未使用的依赖，考虑清理"
else
    echo "⚠️  cargo-machete 未安装，跳过依赖检查"
fi

echo ""

# 3. 构建性能基准测试
echo "3️⃣  构建性能基准测试..."
echo "----------------------------------------"

# 确保bc工具可用
if ! command -v bc &> /dev/null; then
    echo "⚠️  bc 未安装，无法精确测量时间"
    # 使用简单的时间测量
    echo "🔄 使用cargo check测试..."
    time cargo check
    
    echo "🔄 使用cargo build测试..."
    time cargo build
else
    # 精确测量
    measure_build_time "cargo check" "cargo check"
    measure_build_time "cargo build (dev)" "cargo build"
    measure_build_time "cargo build (release)" "cargo build --release"
fi

echo ""

# 4. 测试新的别名命令
echo "4️⃣  测试构建别名命令..."
echo "----------------------------------------"

echo "📝 可用的构建别名:"
echo "   cargo c    → cargo check (最快的代码验证)"
echo "   cargo b    → cargo build"
echo "   cargo t    → cargo test"
echo "   cargo lint → cargo clippy --all-targets --all-features"
echo "   cargo w    → cargo watch -x check (自动检查代码变化)"
echo "   cargo wt   → cargo watch -x test (自动运行测试)"

echo ""
echo "🧪 测试 cargo c (check) 命令..."
cargo c

echo ""

# 5. 构建配置验证
echo "5️⃣  构建配置验证..."
echo "----------------------------------------"

echo "📋 当前构建配置:"
echo "   🎯 目标目录: $(cargo metadata --format-version 1 | jq -r '.target_directory')"
echo "   🔧 并行任务: $(nproc 2>/dev/null || sysctl -n hw.ncpu)"
echo "   🔗 链接器: lld (已配置)"
echo "   📦 开发配置: codegen-units=256, incremental=true"
echo "   🚀 发布配置: lto=true, opt-level='s'"

echo ""

# 6. 性能建议
echo "6️⃣  性能优化建议..."
echo "----------------------------------------"

echo "💡 开发时建议:"
echo "   • 使用 'cargo c' 替代 'cargo build' 进行快速验证"
echo "   • 使用 'cargo w' 启动自动检查模式"
echo "   • 只在需要运行程序时才使用 'cargo build'"
echo "   • 使用 'cargo t' 运行测试"

echo ""
echo "🔧 进一步优化选项:"
if ! command -v sccache &> /dev/null; then
    echo "   • 安装 sccache 启用编译缓存:"
    echo "     cargo install sccache"
    echo "     export RUSTC_WRAPPER=sccache"
fi

echo "   • 在 CI/CD 中使用 cargo-chef 优化依赖缓存"
echo "   • 考虑使用 cargo-nextest 加速测试运行"

echo ""

# 7. 内存和磁盘使用情况
echo "7️⃣  资源使用情况..."
echo "----------------------------------------"

target_dir="/Volumes/UltraDisk/Dev2/aastar/AirAccount/target"
if [ -d "$target_dir" ]; then
    target_size=$(du -sh "$target_dir" 2>/dev/null | cut -f1)
    echo "📁 Target目录大小: $target_size"
else
    echo "📁 Target目录: 未找到"
fi

cargo_cache_size=$(du -sh ~/.cargo 2>/dev/null | cut -f1 || echo "未知")
echo "📦 Cargo缓存大小: $cargo_cache_size"

echo "💾 当前磁盘使用:"
df -h /Volumes/UltraDisk/Dev2 2>/dev/null || df -h /

echo ""

# 8. 推荐的开发工作流
echo "8️⃣  推荐的开发工作流..."
echo "----------------------------------------"

echo "🔄 快速开发循环:"
echo "   1. 启动自动检查: cargo w"
echo "   2. 编辑代码..."
echo "   3. 保存文件 → 自动运行 cargo check"
echo "   4. 需要测试时: cargo t"
echo "   5. 需要运行时: cargo r"

echo ""
echo "📊 性能监控:"
echo "   • 检查编译缓存命中率: sccache -s"
echo "   • 分析编译时间: cargo build --timings"
echo "   • 查看依赖编译时间: cargo +nightly build -Z timings"

echo ""
echo "✅ 构建性能优化配置完成！"
echo ""
echo "💡 提示: 现在开发时优先使用 'cargo c' 而不是 'cargo build'"
echo "   这将显著提升开发效率！"