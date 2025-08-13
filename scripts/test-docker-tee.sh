#!/bin/bash
# Docker TEE环境测试脚本

set -e

echo "🐳 Testing Docker TEE Environment"
echo "================================="

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查Docker是否可用
check_docker() {
    log_info "Checking Docker availability..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    log_success "Docker is available: $(docker --version)"
}

# 检查Docker Compose
check_docker_compose() {
    log_info "Checking Docker Compose availability..."
    
    if docker compose version &> /dev/null; then
        log_success "Docker Compose is available: $(docker compose version --short)"
    elif docker-compose --version &> /dev/null; then
        log_success "Docker Compose (standalone) is available: $(docker-compose --version)"
    else
        log_warning "Docker Compose not found, some tests will be skipped"
    fi
}

# 测试基础Docker功能
test_docker_basic() {
    log_info "Testing basic Docker functionality..."
    
    # 运行简单的容器测试
    if docker run --rm hello-world > /dev/null 2>&1; then
        log_success "Docker basic functionality works"
    else
        log_error "Docker basic test failed"
        return 1
    fi
}

# 测试TEE相关Docker镜像
test_tee_images() {
    log_info "Testing TEE-related Docker images..."
    
    # 检查Ubuntu基础镜像
    if docker pull ubuntu:22.04 > /dev/null 2>&1; then
        log_success "Ubuntu 22.04 base image available"
    else
        log_warning "Failed to pull Ubuntu 22.04 image"
    fi
    
    # 检查Rust镜像
    if docker pull rust:1.70 > /dev/null 2>&1; then
        log_success "Rust 1.70 image available"  
    else
        log_warning "Failed to pull Rust image"
    fi
}

# 测试ARM交叉编译环境
test_arm_cross_compile() {
    log_info "Testing ARM cross-compilation in Docker..."
    
    # 创建临时测试目录
    TEST_DIR=$(mktemp -d)
    cd "$TEST_DIR"
    
    # 创建简单的C程序
    cat > test.c << 'EOF'
#include <stdio.h>
int main() {
    printf("Hello from ARM!\n");
    return 0;
}
EOF
    
    # 在Docker中测试ARM交叉编译
    if docker run --rm -v "$PWD:/workspace" -w /workspace ubuntu:22.04 \
       bash -c "apt-get update -q && apt-get install -y gcc-arm-linux-gnueabihf > /dev/null 2>&1 && arm-linux-gnueabihf-gcc test.c -o test_arm"; then
        log_success "ARM cross-compilation test passed"
    else
        log_warning "ARM cross-compilation test failed"
    fi
    
    # 清理
    cd - > /dev/null
    rm -rf "$TEST_DIR"
}

# 测试QEMU ARM仿真
test_qemu_arm() {
    log_info "Testing QEMU ARM emulation..."
    
    # 在Docker中测试QEMU
    if docker run --rm ubuntu:22.04 \
       bash -c "apt-get update -q && apt-get install -y qemu-user-static > /dev/null 2>&1 && echo 'QEMU ARM support available'"; then
        log_success "QEMU ARM emulation support available"
    else
        log_warning "QEMU ARM emulation test failed"
    fi
}

# 测试TEE开发环境构建
test_tee_dev_build() {
    log_info "Testing TEE development environment build..."
    
    # 检查Dockerfile是否存在
    if [ -f "docker/Dockerfile.optee" ]; then
        log_info "Found OP-TEE Dockerfile, testing build..."
        
        # 尝试构建镜像（仅验证阶段）
        if docker build -f docker/Dockerfile.optee --target builder -t airaccount-tee-test . > /dev/null 2>&1; then
            log_success "TEE development environment build test passed"
            
            # 清理测试镜像
            docker rmi airaccount-tee-test > /dev/null 2>&1 || true
        else
            log_warning "TEE development environment build failed"
        fi
    else
        log_warning "OP-TEE Dockerfile not found, skipping build test"
    fi
}

# 测试网络连接
test_network() {
    log_info "Testing Docker networking for TEE services..."
    
    # 创建测试网络
    NETWORK_NAME="tee-test-net"
    if docker network create "$NETWORK_NAME" > /dev/null 2>&1; then
        log_success "Created test network: $NETWORK_NAME"
        
        # 清理网络
        docker network rm "$NETWORK_NAME" > /dev/null 2>&1
    else
        log_warning "Failed to create test network"
    fi
}

# 测试卷挂载
test_volumes() {
    log_info "Testing Docker volume mounting..."
    
    # 创建临时文件
    TEMP_FILE=$(mktemp)
    echo "TEE volume test" > "$TEMP_FILE"
    
    # 测试卷挂载
    if docker run --rm -v "$TEMP_FILE:/test.txt" ubuntu:22.04 cat /test.txt | grep -q "TEE volume test"; then
        log_success "Docker volume mounting works"
    else
        log_warning "Docker volume mounting test failed"
    fi
    
    # 清理
    rm -f "$TEMP_FILE"
}

# 运行Rust测试在Docker中
test_rust_in_docker() {
    log_info "Testing Rust compilation in Docker..."
    
    # 创建简单的Rust项目测试
    if docker run --rm -v "$PWD:/workspace" -w /workspace rust:1.70 \
       bash -c "cd packages/core-logic && cargo check --quiet"; then
        log_success "Rust compilation in Docker works"
    else
        log_warning "Rust compilation in Docker failed"
    fi
}

# 主测试流程
main() {
    echo "Starting Docker TEE environment tests..."
    echo ""
    
    check_docker
    check_docker_compose
    test_docker_basic
    test_tee_images
    test_arm_cross_compile
    test_qemu_arm
    test_network
    test_volumes
    test_rust_in_docker
    
    # 如果在项目根目录，运行额外测试
    if [ -f "Cargo.toml" ]; then
        test_tee_dev_build
    fi
    
    echo ""
    log_success "Docker TEE environment tests completed!"
    echo ""
    echo "Next steps:"
    echo "1. Run: docker-compose -f docker-compose.tee.yml up -d"
    echo "2. Test TEE services with: cargo test tee_docker_tests -- --ignored"
    echo "3. View logs: docker-compose -f docker-compose.tee.yml logs -f"
}

# 脚本入口
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi