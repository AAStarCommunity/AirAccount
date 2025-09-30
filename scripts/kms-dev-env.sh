#!/bin/bash
# KMS开发环境管理脚本 - STD模式

set -e

# 配置
DOCKER_IMAGE="teaclave/teaclave-trustzone-emulator-std-optee-4.5.0-expand-memory:latest"
CONTAINER_NAME="teaclave_dev_env"
SDK_PATH="$(cd "$(dirname "$0")/.." && pwd)/third_party/teaclave-trustzone-sdk"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Step 1: 拉取Docker镜像
step1_pull_image() {
    log_info "Step 1: 拉取STD模式Docker镜像..."
    if docker images | grep -q "teaclave-trustzone-emulator-std-optee-4.5.0"; then
        log_info "镜像已存在，跳过下载"
    else
        docker pull $DOCKER_IMAGE
    fi
    log_info "✅ Step 1 完成"
}

# Step 2: 启动容器
step2_start_container() {
    log_info "Step 2: 启动STD模式开发容器..."

    # 检查容器是否已存在
    if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
        log_warn "容器已存在，正在删除..."
        docker rm -f $CONTAINER_NAME
    fi

    # 启动新容器
    docker run -d \
        --name $CONTAINER_NAME \
        -v "$SDK_PATH:/root/teaclave_sdk_src" \
        -w /root/teaclave_sdk_src \
        $DOCKER_IMAGE \
        tail -f /dev/null

    # 等待容器启动
    sleep 2

    # 验证环境
    log_info "验证STD环境..."
    docker exec $CONTAINER_NAME bash -l -c "switch_config --status"

    # 检查并初始化rust/libc依赖（STD模式必需）
    log_info "检查STD模式依赖（rust/libc）..."
    if ! docker exec $CONTAINER_NAME bash -l -c "[ -d /root/teaclave_sdk_src/rust/rust ] && [ -d /root/teaclave_sdk_src/rust/libc ]"; then
        log_warn "rust/libc依赖不存在，开始初始化..."
        docker exec $CONTAINER_NAME bash -l -c "cd /root/teaclave_sdk_src && ./setup_std_dependencies.sh"
        log_info "✅ STD依赖初始化完成"
    else
        log_info "✅ STD依赖已存在，跳过初始化"
    fi

    log_info "✅ Step 2 完成 - 容器已启动（STD模式）"
}

# Step 3: 构建KMS项目
step3_build_kms() {
    log_info "Step 3: 构建KMS项目..."
    docker exec $CONTAINER_NAME bash -l -c "cd /root/teaclave_sdk_src/projects/web3/kms && make clean && make"
    log_info "✅ Step 3 完成 - KMS构建成功"
}

# Step 4: 同步构建产物到QEMU共享目录
step4_sync_artifacts() {
    log_info "Step 4: 同步构建产物到QEMU共享目录..."
    docker exec $CONTAINER_NAME bash -l -c "
        mkdir -p /opt/teaclave/shared && \
        cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms /opt/teaclave/shared/ && \
        cp /root/teaclave_sdk_src/projects/web3/kms/host/target/aarch64-unknown-linux-gnu/release/kms-api-server /opt/teaclave/shared/ && \
        cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/teaclave/shared/ && \
        ls -lh /opt/teaclave/shared/
    "
    log_info "✅ Step 4 完成 - 构建产物已同步（CLI + API Server）"
}

# 显示使用帮助
show_usage() {
    cat << EOF
KMS开发环境管理脚本 (STD模式)

用法: $0 <command>

命令:
  all             执行所有步骤（1-4）
  pull            Step 1: 拉取STD模式Docker镜像
  start           Step 2: 启动开发容器
  build           Step 3: 构建KMS项目
  sync            Step 4: 同步构建产物
  stop            停止容器
  restart         重启容器
  shell           进入容器shell
  status          查看容器状态
  help            显示此帮助信息

示例:
  # 完整初始化
  $0 all

  # 仅重新构建并同步
  $0 build && $0 sync

  # 进入容器调试
  $0 shell

注意:
  - 使用STD模式镜像（支持xargo + Rust std）
  - 容器会自动配置为std/aarch64模式
  - 构建产物会同步到/opt/teaclave/shared/
EOF
}

# 停止容器
stop_container() {
    log_info "停止容器..."
    docker stop $CONTAINER_NAME 2>/dev/null || log_warn "容器未运行"
    log_info "✅ 容器已停止"
}

# 重启容器
restart_container() {
    log_info "重启容器..."
    docker restart $CONTAINER_NAME
    log_info "✅ 容器已重启"
}

# 进入容器shell
enter_shell() {
    log_info "进入容器shell..."
    docker exec -it $CONTAINER_NAME bash -l
}

# 查看状态
show_status() {
    log_info "容器状态:"
    docker ps -a --filter "name=$CONTAINER_NAME" --format "table {{.Names}}\t{{.Status}}\t{{.Image}}"

    if docker ps --filter "name=$CONTAINER_NAME" | grep -q "$CONTAINER_NAME"; then
        log_info "\n环境配置:"
        docker exec $CONTAINER_NAME bash -l -c "switch_config --status"
    fi
}

# 主逻辑
case "${1:-help}" in
    all)
        step1_pull_image
        step2_start_container
        step3_build_kms
        step4_sync_artifacts
        log_info "🎉 所有步骤完成！"
        log_info "下一步: 运行QEMU监听器和启动器"
        ;;
    pull)
        step1_pull_image
        ;;
    start)
        step2_start_container
        ;;
    build)
        step3_build_kms
        ;;
    sync)
        step4_sync_artifacts
        ;;
    stop)
        stop_container
        ;;
    restart)
        restart_container
        ;;
    shell)
        enter_shell
        ;;
    status)
        show_status
        ;;
    help|--help|-h)
        show_usage
        ;;
    *)
        log_error "未知命令: $1"
        show_usage
        exit 1
        ;;
esac