#!/bin/bash
# TEE环境健康检查脚本

set -e

echo "🔍 TEE Environment Health Check"
echo "==============================="

# 颜色定义
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 健康检查结果
CHECKS_PASSED=0
CHECKS_TOTAL=0

# 检查函数
check_status() {
    local check_name="$1"
    local check_result="$2"
    
    ((CHECKS_TOTAL++))
    
    if [ "$check_result" -eq 0 ]; then
        echo -e "${GREEN}✅ $check_name${NC}"
        ((CHECKS_PASSED++))
    else
        echo -e "${RED}❌ $check_name${NC}"
    fi
}

# 检查核心系统组件
echo "1. Core System Components"
echo "------------------------"

# 检查Rust工具链
if cargo --version > /dev/null 2>&1; then
    check_status "Rust toolchain available" 0
else
    check_status "Rust toolchain available" 1
fi

# 检查交叉编译工具链
if command -v arm-linux-gnueabihf-gcc > /dev/null 2>&1; then
    check_status "ARM cross-compiler available" 0
else
    check_status "ARM cross-compiler available" 1
fi

# 检查QEMU
if command -v qemu-system-arm > /dev/null 2>&1; then
    check_status "QEMU ARM system available" 0
else
    check_status "QEMU ARM system available" 1
fi

echo ""
echo "2. TEE Development Environment"
echo "-----------------------------"

# 检查OP-TEE目录
if [ -d "third_party/optee_os" ]; then
    check_status "OP-TEE OS directory exists" 0
else
    check_status "OP-TEE OS directory exists" 1
fi

if [ -d "third_party/optee_client" ]; then
    check_status "OP-TEE client directory exists" 0
else
    check_status "OP-TEE client directory exists" 1
fi

# 检查Teaclave SDK
if [ -d "third_party/incubator-teaclave-trustzone-sdk" ]; then
    check_status "Teaclave TrustZone SDK available" 0
else
    check_status "Teaclave TrustZone SDK available" 1
fi

echo ""
echo "3. Project Structure"
echo "-------------------"

# 检查核心逻辑包
if [ -f "packages/core-logic/Cargo.toml" ]; then
    check_status "Core logic package exists" 0
else
    check_status "Core logic package exists" 1
fi

# 检查TEE模块
if [ -f "packages/core-logic/src/tee/mod.rs" ]; then
    check_status "TEE module exists" 0
else
    check_status "TEE module exists" 1
fi

# 检查测试文件
if [ -f "packages/core-logic/tests/integration_tee_basic.rs" ]; then
    check_status "TEE integration tests exist" 0
else
    check_status "TEE integration tests exist" 1
fi

echo ""
echo "4. Build Environment"  
echo "-------------------"

# 检查构建是否成功
if cd packages/core-logic && cargo check --quiet > /dev/null 2>&1; then
    check_status "Core logic builds successfully" 0
    cd - > /dev/null
else
    check_status "Core logic builds successfully" 1
    cd - > /dev/null
fi

# 检查测试是否运行
if cd packages/core-logic && cargo test --test integration_tee_basic --quiet > /dev/null 2>&1; then
    check_status "TEE integration tests pass" 0
    cd - > /dev/null
else
    check_status "TEE integration tests pass" 1
    cd - > /dev/null
fi

echo ""
echo "5. Docker Environment"
echo "--------------------"

# 检查Docker是否可用
if command -v docker > /dev/null 2>&1 && docker info > /dev/null 2>&1; then
    check_status "Docker is available and running" 0
else
    check_status "Docker is available and running" 1
fi

# 检查Docker Compose配置
if [ -f "docker-compose.tee.yml" ]; then
    check_status "TEE Docker Compose config exists" 0
else
    check_status "TEE Docker Compose config exists" 1
fi

# 检查Dockerfile
if [ -f "docker/Dockerfile.optee" ]; then
    check_status "OP-TEE Dockerfile exists" 0
else
    check_status "OP-TEE Dockerfile exists" 1
fi

echo ""
echo "6. Network and Ports"
echo "-------------------"

# 检查TEE服务端口是否可用
if ! netstat -an 2>/dev/null | grep -q ":5000 "; then
    check_status "TEE service port (5000) is available" 0
else
    check_status "TEE service port (5000) is available" 1
fi

# 检查管理API端口是否可用  
if ! netstat -an 2>/dev/null | grep -q ":9000 "; then
    check_status "Management API port (9000) is available" 0
else
    check_status "Management API port (9000) is available" 1
fi

echo ""
echo "Health Check Summary"
echo "==================="

# 计算健康分数
if [ $CHECKS_TOTAL -gt 0 ]; then
    HEALTH_PERCENTAGE=$(( (CHECKS_PASSED * 100) / CHECKS_TOTAL ))
else
    HEALTH_PERCENTAGE=0
fi

echo "Checks passed: $CHECKS_PASSED / $CHECKS_TOTAL"
echo "Health score: $HEALTH_PERCENTAGE%"

# 根据健康分数决定退出状态
if [ $HEALTH_PERCENTAGE -ge 80 ]; then
    echo -e "${GREEN}🎉 TEE environment is healthy!${NC}"
    exit 0
elif [ $HEALTH_PERCENTAGE -ge 60 ]; then
    echo -e "${YELLOW}⚠️ TEE environment has some issues but is mostly functional${NC}"
    exit 0
else
    echo -e "${RED}❌ TEE environment has significant issues${NC}"
    exit 1
fi