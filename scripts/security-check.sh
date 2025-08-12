#!/bin/bash

# Rust Security Check Script
# 用于检测和防范Rust项目中的后门和安全漏洞

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "========================================="
echo "   Rust Security Check for AirAccount   "
echo "========================================="

# 检查是否在项目根目录
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}错误: 请在项目根目录运行此脚本${NC}"
    exit 1
fi

ISSUES_FOUND=0

# 1. 检查build.rs文件中的可疑模式
echo -e "\n${YELLOW}[1/8] 检查build.rs中的可疑模式...${NC}"
if find . -name "build.rs" -type f 2>/dev/null | head -n 1 | grep -q .; then
    SUSPICIOUS_BUILD=$(find . -name "build.rs" -type f -exec grep -l -E "Command|Process|spawn|execute|download|curl|wget|reqwest|hyper" {} \; 2>/dev/null || true)
    if [ ! -z "$SUSPICIOUS_BUILD" ]; then
        echo -e "${RED}⚠ 发现可疑的build.rs文件:${NC}"
        echo "$SUSPICIOUS_BUILD"
        echo -e "${YELLOW}  建议: 手动审查这些文件是否包含恶意代码${NC}"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    else
        echo -e "${GREEN}✓ build.rs文件检查通过${NC}"
    fi
else
    echo -e "${GREEN}✓ 未发现build.rs文件${NC}"
fi

# 2. 检查过程宏依赖
echo -e "\n${YELLOW}[2/8] 检查过程宏依赖...${NC}"
PROC_MACROS=$(cargo tree 2>/dev/null | grep "proc-macro" || true)
if [ ! -z "$PROC_MACROS" ]; then
    echo -e "${YELLOW}⚠ 发现以下过程宏依赖:${NC}"
    echo "$PROC_MACROS" | head -5
    echo -e "${YELLOW}  建议: 确认这些过程宏来源可信${NC}"
else
    echo -e "${GREEN}✓ 未发现过程宏依赖${NC}"
fi

# 3. 检查可疑的网络请求代码
echo -e "\n${YELLOW}[3/8] 检查可疑的网络请求...${NC}"
NETWORK_PATTERNS=$(grep -r -E "reqwest|hyper|curl|download|fetch.*http" --include="*.rs" . 2>/dev/null | head -5 || true)
if [ ! -z "$NETWORK_PATTERNS" ]; then
    echo -e "${YELLOW}⚠ 发现网络请求相关代码:${NC}"
    echo "$NETWORK_PATTERNS" | head -3
    echo -e "${YELLOW}  建议: 审查这些网络请求是否必要和安全${NC}"
fi

# 4. 检查环境变量访问
echo -e "\n${YELLOW}[4/8] 检查敏感环境变量访问...${NC}"
ENV_ACCESS=$(grep -r "std::env::" --include="*.rs" . 2>/dev/null | grep -E "SECRET|KEY|TOKEN|PASSWORD|API" || true)
if [ ! -z "$ENV_ACCESS" ]; then
    echo -e "${RED}⚠ 发现敏感环境变量访问:${NC}"
    echo "$ENV_ACCESS" | head -3
    echo -e "${YELLOW}  建议: 确保环境变量不被泄露${NC}"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
else
    echo -e "${GREEN}✓ 未发现可疑的环境变量访问${NC}"
fi

# 5. 检查Cargo.toml中的可疑依赖
echo -e "\n${YELLOW}[5/8] 检查可疑的依赖名称...${NC}"
SUSPICIOUS_DEPS=""
for dep in "rands" "rustdecimal" "lazystatic" "oncecell" "serd" "envlogger" "postgress" "if-cfg" "xrvrv"; do
    if grep -q "\"$dep\"" Cargo.toml 2>/dev/null; then
        SUSPICIOUS_DEPS="$SUSPICIOUS_DEPS $dep"
    fi
done

if [ ! -z "$SUSPICIOUS_DEPS" ]; then
    echo -e "${RED}⚠ 发现已知的可疑依赖: $SUSPICIOUS_DEPS${NC}"
    echo -e "${YELLOW}  建议: 立即检查这些依赖是否为恶意包${NC}"
    ISSUES_FOUND=$((ISSUES_FOUND + 1))
else
    echo -e "${GREEN}✓ 未发现已知的恶意依赖${NC}"
fi

# 6. 检查Git依赖
echo -e "\n${YELLOW}[6/8] 检查Git依赖...${NC}"
GIT_DEPS=$(grep -E "git\s*=" Cargo.toml || true)
if [ ! -z "$GIT_DEPS" ]; then
    echo -e "${YELLOW}⚠ 发现Git依赖:${NC}"
    echo "$GIT_DEPS"
    echo -e "${YELLOW}  建议: 优先使用crates.io上的正式版本${NC}"
fi

# 7. 检查重复依赖
echo -e "\n${YELLOW}[7/8] 检查重复依赖...${NC}"
DUPLICATES=$(cargo tree --duplicates 2>/dev/null || true)
if [ ! -z "$DUPLICATES" ]; then
    echo -e "${YELLOW}⚠ 发现重复的依赖包:${NC}"
    echo "$DUPLICATES" | head -5
    echo -e "${YELLOW}  建议: 统一依赖版本以减少攻击面${NC}"
fi

# 8. 运行cargo-audit（如果已安装）
echo -e "\n${YELLOW}[8/8] 运行安全审计工具...${NC}"
if command -v cargo-audit &> /dev/null; then
    echo "运行cargo-audit..."
    cargo audit 2>/dev/null || {
        echo -e "${YELLOW}⚠ cargo-audit发现问题，请查看上述输出${NC}"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
    }
else
    echo -e "${YELLOW}⚠ cargo-audit未安装${NC}"
    echo "  建议安装: cargo install cargo-audit"
fi

# 总结
echo -e "\n========================================="
if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✅ 安全检查完成，未发现严重问题${NC}"
else
    echo -e "${RED}⚠️ 安全检查完成，发现 $ISSUES_FOUND 个潜在问题${NC}"
    echo -e "${YELLOW}请根据上述建议进行处理${NC}"
fi
echo "========================================="

# 提供快速修复建议
if [ $ISSUES_FOUND -gt 0 ]; then
    echo -e "\n${YELLOW}快速修复建议:${NC}"
    echo "1. 安装安全工具:"
    echo "   cargo install cargo-audit cargo-deny cargo-outdated"
    echo "2. 创建deny.toml配置文件"
    echo "3. 审查所有build.rs和过程宏"
    echo "4. 考虑使用隔离的开发环境"
fi

exit $ISSUES_FOUND