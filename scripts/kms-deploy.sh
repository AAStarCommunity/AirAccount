#!/bin/bash
# KMS快速部署脚本 - 同步源码、构建并部署到QEMU

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_step() {
    echo -e "\n${BLUE}==>${NC} $1"
}

# 项目路径
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KMS_DEV_DIR="$PROJECT_ROOT/kms"
KMS_SDK_DIR="$PROJECT_ROOT/third_party/teaclave-trustzone-sdk/projects/web3/kms"

# 检查容器是否运行
if ! docker ps | grep -q teaclave_dev_env; then
    echo -e "${RED}[ERROR]${NC} 容器未运行！请先执行: ./scripts/kms-dev-env.sh start"
    exit 1
fi

# Step 0: 同步开发源码到SDK
log_step "0/4 同步开发源码到SDK..."
log_info "从 kms/ 同步到 third_party/teaclave-trustzone-sdk/projects/web3/kms/"
rsync -av --delete "$KMS_DEV_DIR/" "$KMS_SDK_DIR/"
log_info "✅ 源码同步完成"

# 检查STD模式依赖
log_info "检查STD模式依赖（rust/libc）..."
if ! docker exec teaclave_dev_env bash -l -c "[ -d /root/teaclave_sdk_src/rust/rust ] && [ -d /root/teaclave_sdk_src/rust/libc ]"; then
    log_warn "rust/libc依赖不存在，开始初始化（首次运行或SDK被重置后需要）..."
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src && ./setup_std_dependencies.sh"
    log_info "✅ STD依赖初始化完成"
else
    log_info "✅ STD依赖已存在"
fi

# 是否清理构建
CLEAN_BUILD=${1:-no}
if [ "$CLEAN_BUILD" = "clean" ]; then
    log_step "1/4 清理旧构建..."
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make clean"
else
    log_info "跳过清理（增量构建），如需完整构建请运行: $0 clean"
fi

# 构建KMS
log_step "2/4 构建KMS项目（Host + TA）..."
docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make 2>&1 | grep -E 'Compiling|Finished|SIGN|error' || make"

if [ $? -ne 0 ]; then
    log_warn "构建失败，查看完整日志:"
    docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make"
    exit 1
fi

# 同步到QEMU共享目录
log_step "3/4 部署到QEMU共享目录..."
docker exec teaclave_dev_env bash -l -c "
    mkdir -p /opt/teaclave/shared/ta && \
    mkdir -p /opt/teaclave/shared/plugin && \
    cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms /opt/teaclave/shared/kms && \
    cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/kms-api-server && \
    cp /root/teaclave_sdk_src/projects/web3/kms/host/kms-test-page.html /opt/teaclave/shared/kms-test-page.html 2>/dev/null || true && \
    cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/teaclave/shared/ta/ && \
    ls -lh /opt/teaclave/shared/ta/ && \
    echo '---' && \
    ls -lh /opt/teaclave/shared/ | grep -E 'kms|\.html$'
"

log_step "4/5 验证部署..."
docker exec teaclave_dev_env bash -l -c "ls -lh /opt/teaclave/shared/ | grep -E 'kms|\.ta$'"

log_step "5/5 重启 QEMU 内的 API Server..."
# 检查 QEMU 是否在运行
if docker exec teaclave_dev_env ps aux | grep -q "[q]emu-system-aarch64"; then
    log_info "检测到 QEMU 正在运行，尝试重启 API Server..."

    # 通过 /opt/teaclave/shared 创建重启脚本
    docker exec teaclave_dev_env bash -c "cat > /opt/teaclave/shared/.restart_api.sh << 'EOF'
#!/bin/sh
# 停止旧的 API Server
pkill kms-api-server 2>/dev/null || true
sleep 1

# 启动新版本
cd /root/shared
./kms-api-server > kms-api.log 2>&1 &

sleep 2
ps aux | grep kms-api-server | grep -v grep
echo 'API Server restarted'
EOF
chmod +x /opt/teaclave/shared/.restart_api.sh"

    # 尝试通过 socat 执行重启（如果 QEMU 可访问）
    timeout 5 docker exec teaclave_dev_env bash -c "echo 'cd /root/shared && sh .restart_api.sh' | socat - TCP:localhost:54320" 2>/dev/null || {
        log_warn "无法通过 socat 自动重启，请手动执行以下命令："
        echo -e "${YELLOW}  docker exec teaclave_dev_env bash -c \"echo 'cd /root/shared && sh .restart_api.sh' | socat - TCP:localhost:54320\"${NC}"
    }
else
    log_warn "QEMU 未运行，跳过 API Server 重启"
    echo -e "${YELLOW}  启动 QEMU 后，API Server 会自动启动${NC}"
fi

echo ""
log_info "✅ 部署完成！"
echo ""
echo -e "${BLUE}📊 当前状态：${NC}"
echo "  ✅ 代码已同步到 SDK"
echo "  ✅ 编译完成（Host CA + TA）"
echo "  ✅ 二进制文件已部署到 /opt/teaclave/shared"
if docker exec teaclave_dev_env ps aux | grep -q "[q]emu-system-aarch64"; then
    echo "  🔄 API Server 重启中..."
else
    echo "  ⏸️  QEMU 未运行，等待启动"
fi
echo ""
echo -e "${BLUE}开发流程：${NC}"
echo "  📝 修改代码: 编辑 kms/ 目录"
echo "  🚀 部署: ./scripts/kms-deploy.sh"
echo "  🧪 测试: curl http://localhost:3000/health"
echo ""