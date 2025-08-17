#!/bin/bash
# Enhanced Security Report Generator
# 增强的安全报告生成器

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m'

REPORT_FILE="security-report-$(date +%Y%m%d-%H%M%S).md"
LOG_FILE="/tmp/security-check-detailed.log"

echo -e "${BLUE}🛡️ 生成AirAccount安全报告${NC}"
echo "================================================"

# 创建报告头部
cat > "$REPORT_FILE" << EOF
# AirAccount Security Report
生成时间: $(date '+%Y-%m-%d %H:%M:%S')
项目版本: $(git describe --tags --always 2>/dev/null || echo "unknown")

## 执行摘要

EOF

# 运行安全检查
echo -e "${YELLOW}🔍 执行安全扫描...${NC}"

# 1. Cargo Audit
echo "### 依赖安全审计" >> "$REPORT_FILE"
if command -v cargo-audit >/dev/null 2>&1; then
    echo -e "${BLUE}运行 cargo audit...${NC}"
    if cargo audit 2>/dev/null >> "$LOG_FILE"; then
        echo "✅ **无已知安全漏洞**" >> "$REPORT_FILE"
    else
        echo "⚠️ **发现以下安全问题:**" >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
        cargo audit 2>&1 | tail -n 20 >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
    fi
else
    echo "⚠️ cargo-audit 未安装，跳过依赖审计" >> "$REPORT_FILE"
fi

echo "" >> "$REPORT_FILE"

# 2. 依赖分析
echo "### 依赖分析" >> "$REPORT_FILE"
echo -e "${BLUE}分析项目依赖...${NC}"

# 只分析packages目录，避免第三方代码导致死循环
TOTAL_DEPS=$(find packages/ -name "Cargo.toml" -exec grep -c "^\[dependencies\]" {} \; 2>/dev/null | awk '{sum += $1} END {print sum}' || echo "0")
TOTAL_BUILD_DEPS=$(find packages/ -name "Cargo.toml" -exec grep -c "^\[build-dependencies\]" {} \; 2>/dev/null | awk '{sum += $1} END {print sum}' || echo "0")

cat >> "$REPORT_FILE" << EOF
- **总依赖数**: $TOTAL_DEPS
- **构建依赖数**: $TOTAL_BUILD_DEPS
- **第三方模块数**: $(find . -name "third_party" -type d | wc -l)

EOF

# 3. 代码安全检查
echo "### 代码安全检查" >> "$REPORT_FILE"
echo -e "${BLUE}检查敏感模式...${NC}"

SENSITIVE_PATTERNS=0
# 只检查源代码，排除target目录和第三方代码
if grep -r "unsafe" --include="*.rs" packages/ 2>/dev/null | grep -v "test" | grep -v "target/" | head -5 > /tmp/unsafe_patterns; then
    SENSITIVE_PATTERNS=$(wc -l < /tmp/unsafe_patterns)
    if [ "$SENSITIVE_PATTERNS" -gt 0 ]; then
        echo "⚠️ **在项目源码中发现 $SENSITIVE_PATTERNS 个 unsafe 代码块:**" >> "$REPORT_FILE"
        echo '```rust' >> "$REPORT_FILE"
        head -5 /tmp/unsafe_patterns >> "$REPORT_FILE"
        echo '```' >> "$REPORT_FILE"
    fi
else
    echo "✅ **项目源码中未发现 unsafe 代码块**" >> "$REPORT_FILE"
fi

echo "" >> "$REPORT_FILE"

# 4. 构建脚本检查
echo "### 构建脚本安全检查" >> "$REPORT_FILE"
echo -e "${BLUE}检查 build.rs 文件...${NC}"

# 只检查项目源码，排除第三方和备份文件
BUILD_SCRIPTS=$(find packages/ -name "build.rs" 2>/dev/null | grep -v "backup" | wc -l)
if [ "$BUILD_SCRIPTS" -gt 0 ]; then
    echo "⚠️ **在项目中发现 $BUILD_SCRIPTS 个构建脚本需要审查:**" >> "$REPORT_FILE"
    find packages/ -name "build.rs" 2>/dev/null | grep -v "backup" | while read file; do
        echo "- \`$file\`" >> "$REPORT_FILE"
    done
else
    echo "✅ **项目中未发现构建脚本**" >> "$REPORT_FILE"
fi

echo "" >> "$REPORT_FILE"

# 5. 许可证合规检查
echo "### 许可证合规性" >> "$REPORT_FILE"
echo -e "${BLUE}检查许可证兼容性...${NC}"

if command -v cargo-license >/dev/null 2>&1; then
    LICENSES=$(cargo license 2>/dev/null | grep -v "License" | awk '{print $NF}' | sort | uniq -c | sort -nr)
    echo "**依赖许可证分布:**" >> "$REPORT_FILE"
    echo '```' >> "$REPORT_FILE"
    echo "$LICENSES" >> "$REPORT_FILE"
    echo '```' >> "$REPORT_FILE"
else
    echo "⚠️ cargo-license 未安装，跳过许可证检查" >> "$REPORT_FILE"
fi

echo "" >> "$REPORT_FILE"

# 6. 建议和行动项
echo "### 安全建议" >> "$REPORT_FILE"
cat >> "$REPORT_FILE" << EOF

#### 立即行动项
- [ ] 升级 SQLx 到 0.8.1+ 以修复 RUSTSEC-2024-0363
- [ ] 审查所有 unsafe 代码块的必要性
- [ ] 定期运行 \`cargo audit\` 检查新的安全漏洞

#### 长期改进项  
- [ ] 考虑替换不再维护的依赖包
- [ ] 建立依赖更新策略
- [ ] 配置 dependabot 自动检查更新

#### 监控项
- [ ] 定期审查构建脚本
- [ ] 监控 RSA 密钥操作的时序安全性
- [ ] 跟踪第三方模块的安全更新

EOF

# 7. 生成风险评级
echo "### 总体风险评级" >> "$REPORT_FILE"

# 计算风险评分（满分10分）
RISK_SCORE=0

# 检查cargo audit结果
if grep -q "error.*vulnerabilities found" "$LOG_FILE" 2>/dev/null; then
    VULNS=$(grep -o "[0-9]\+ vulnerabilities found" "$LOG_FILE" | grep -o "[0-9]\+" || echo "0")
    RISK_SCORE=$((RISK_SCORE + VULNS))
fi

# unsafe代码风险 (每个+0.5分)
[ "$SENSITIVE_PATTERNS" -gt 0 ] && RISK_SCORE=$((RISK_SCORE + (SENSITIVE_PATTERNS + 1) / 2))

# 构建脚本风险 (每个+0.5分)
[ "$BUILD_SCRIPTS" -gt 0 ] && RISK_SCORE=$((RISK_SCORE + (BUILD_SCRIPTS + 1) / 2))

# 限制最高分为10
[ "$RISK_SCORE" -gt 10 ] && RISK_SCORE=10

if [ "$RISK_SCORE" -eq 0 ]; then
    RISK_LEVEL="🟢 **低风险**"
elif [ "$RISK_SCORE" -lt 4 ]; then
    RISK_LEVEL="🟡 **中等风险**"
else
    RISK_LEVEL="🔴 **高风险**"
fi

echo "$RISK_LEVEL (评分: $RISK_SCORE/10)" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"

# 结束报告
cat >> "$REPORT_FILE" << EOF
---
*报告由 AirAccount 安全检查工具自动生成*  
*详细日志: $LOG_FILE*
EOF

echo "================================================"
echo -e "${GREEN}✅ 安全报告已生成: $REPORT_FILE${NC}"
echo -e "${BLUE}📊 风险评级: $RISK_LEVEL${NC}"
echo -e "${YELLOW}📋 详细日志: $LOG_FILE${NC}"

# 可选：自动打开报告
if command -v code >/dev/null 2>&1; then
    echo -e "${BLUE}在 VS Code 中打开报告...${NC}"
    code "$REPORT_FILE"
fi