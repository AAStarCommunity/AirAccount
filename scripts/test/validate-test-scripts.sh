#!/bin/bash

# 验证所有测试脚本的路径引用是否正确

echo "🔍 验证测试脚本路径引用"
echo "========================"

# 进入项目根目录
cd "$(dirname "$0")/../.."

# 验证函数
validate_script() {
    local script_path="$1"
    local script_name=$(basename "$script_path")
    
    echo "检查脚本: $script_name"
    
    # 检查脚本是否存在
    if [ ! -f "$script_path" ]; then
        echo "❌ 脚本不存在: $script_path"
        return 1
    fi
    
    # 检查脚本是否可执行
    if [ ! -x "$script_path" ]; then
        echo "⚠️ 脚本不可执行，正在修复..."
        chmod +x "$script_path"
    fi
    
    # 检查可能的路径问题
    local issues=0
    
    # 检查是否有未修复的相对路径
    if grep -q "cd packages/" "$script_path" 2>/dev/null; then
        echo "❌ 发现未修复的路径: cd packages/"
        grep -n "cd packages/" "$script_path"
        ((issues++))
    fi
    
    if grep -q "\./packages" "$script_path" 2>/dev/null; then
        echo "❌ 发现未修复的路径: ./packages"
        grep -n "\./packages" "$script_path"
        ((issues++))
    fi
    
    # 检查Docker文件引用
    if grep -q "Dockerfile\.[a-z]" "$script_path" 2>/dev/null; then
        if ! grep -q "docker/.*/Dockerfile\|../../docker/Dockerfile" "$script_path" 2>/dev/null; then
            echo "❌ 发现未修复的Docker文件引用"
            grep -n "Dockerfile\." "$script_path"
            ((issues++))
        fi
    fi
    
    if [ $issues -eq 0 ]; then
        echo "✅ 路径引用检查通过"
    else
        echo "❌ 发现 $issues 个路径问题"
        return 1
    fi
    
    # 基本语法检查
    if bash -n "$script_path" 2>/dev/null; then
        echo "✅ 语法检查通过"
    else
        echo "❌ 语法错误"
        bash -n "$script_path"
        return 1
    fi
    
    echo ""
    return 0
}

# 验证所有测试脚本
echo "开始验证所有测试脚本..."
echo ""

failed_count=0
total_count=0

# 遍历scripts/test目录中的所有shell脚本
for script in scripts/test/*.sh; do
    if [ -f "$script" ]; then
        ((total_count++))
        if ! validate_script "$script"; then
            ((failed_count++))
        fi
    fi
done

echo "========================"
echo "验证结果汇总:"
echo "总脚本数: $total_count"
echo "通过数: $((total_count - failed_count))"
echo "失败数: $failed_count"

if [ $failed_count -eq 0 ]; then
    echo "🎉 所有测试脚本验证通过！"
    exit 0
else
    echo "❌ 有 $failed_count 个脚本需要修复"
    exit 1
fi