#!/usr/bin/env bash
# qemu/setup.sh — 一次性初始化 QEMU 开发环境
# 支持平台：Apple Silicon (arm64) + OrbStack / Intel Mac + Docker Desktop
#
# 运行一次即可，之后用 make -C qemu start/build/test

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT_DIR="$REPO_ROOT/qemu"

source "$SCRIPT_DIR/lib/log.sh"

DOCKER_IMAGE="teaclave/teaclave-trustzone-emulator-std-optee-4.5.0-expand-memory:latest"
CONTAINER_NAME="teaclave_dev_env"
TEACLAVE_SDK="$REPO_ROOT/third_party/teaclave-trustzone-sdk"
SHARED_DIR="/opt/teaclave/shared"
IMG_DIR="/opt/teaclave/images"
IMG_NAME="x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory"

# ── 平台检测 ─────────────────────────────────────────────────────────────
detect_platform() {
    local arch
    arch="$(uname -m)"
    if [ "$arch" = "arm64" ] || [ "$arch" = "aarch64" ]; then
        echo "arm64"
    else
        echo "x86_64"
    fi
}

HOST_ARCH="$(detect_platform)"
log_info "Host platform: macOS $HOST_ARCH"

# Apple Silicon 需要 --platform linux/amd64 拉取 x86_64 Teaclave 镜像
DOCKER_PLATFORM_FLAG=""
if [ "$HOST_ARCH" = "arm64" ]; then
    DOCKER_PLATFORM_FLAG="--platform linux/amd64"
    log_warn "Apple Silicon detected — using Rosetta-emulated x86_64 container for QEMU"
    log_warn "Performance note: QEMU runs under emulation; use real i.MX 95 hardware for production"
fi

# ── 前置检查 ──────────────────────────────────────────────────────────────
check_prerequisites() {
    log_step "检查前置依赖"

    if ! command -v docker &>/dev/null; then
        log_error "Docker not found. Install OrbStack (recommended) or Docker Desktop."
        exit 1
    fi
    log_info "Docker: $(docker --version | head -1)"

    if ! command -v tmux &>/dev/null; then
        log_warn "tmux not found — single-terminal mode only (no parallel log view)"
        log_warn "Install: brew install tmux"
    else
        log_info "tmux: $(tmux -V)"
    fi

    if [ ! -d "$TEACLAVE_SDK" ]; then
        log_error "Teaclave SDK not found at: $TEACLAVE_SDK"
        log_error "Run: git submodule update --init --recursive"
        exit 1
    fi
    log_info "Teaclave SDK: $TEACLAVE_SDK"
}

# ── 目录结构 ──────────────────────────────────────────────────────────────
setup_directories() {
    log_step "创建目录结构"
    sudo mkdir -p "$SHARED_DIR/ta" "$SHARED_DIR/plugin" "$IMG_DIR"
    sudo chown -R "$(id -u):$(id -g)" /opt/teaclave
    log_info "Created: $SHARED_DIR  $IMG_DIR"
}

# ── 拉取 Docker 镜像 ──────────────────────────────────────────────────────
pull_docker_image() {
    log_step "拉取 Teaclave Docker 镜像"
    if docker images --format "{{.Repository}}:{{.Tag}}" | grep -qF "teaclave-trustzone-emulator-std-optee-4.5.0"; then
        log_info "镜像已存在，跳过下载"
    else
        log_info "Pulling $DOCKER_IMAGE  (this may take 10-30 minutes on first run)..."
        # shellcheck disable=SC2086
        docker pull $DOCKER_PLATFORM_FLAG "$DOCKER_IMAGE"
    fi
}

# ── 解包 QEMU 镜像 ────────────────────────────────────────────────────────
# Teaclave Docker 镜像内含预编译的 QEMU 二进制 + OP-TEE boot files。
# 我们把它们解压到 /opt/teaclave/images/ 供后续启动使用。
extract_qemu_images() {
    log_step "解包 QEMU 镜像文件 (bl1.bin / Image / rootfs.cpio.gz / qemu-system-aarch64)"
    local target_dir="$IMG_DIR/$IMG_NAME"

    if [ -d "$target_dir" ] && [ -f "$target_dir/bl1.bin" ]; then
        log_info "QEMU 镜像已存在：$target_dir，跳过"
        return
    fi

    log_info "启动临时容器提取镜像..."
    # shellcheck disable=SC2086
    EXTRACT_ID=$(docker run $DOCKER_PLATFORM_FLAG -d \
        --name teaclave_extract_tmp \
        "$DOCKER_IMAGE" \
        sleep 60)

    mkdir -p "$target_dir"
    # 镜像在容器内的标准路径
    docker cp "teaclave_extract_tmp:/opt/teaclave/images/$IMG_NAME/." "$target_dir/" 2>/dev/null || {
        log_warn "Standard path not found, trying alternate..."
        docker exec teaclave_extract_tmp find /opt/teaclave -name "bl1.bin" 2>/dev/null | head -3
        log_error "Could not locate QEMU images in container. Run ./qemu/setup.sh --skip-extract and mount manually."
        docker rm -f teaclave_extract_tmp 2>/dev/null || true
        exit 1
    }

    docker rm -f teaclave_extract_tmp 2>/dev/null || true
    log_info "QEMU 镜像已提取到 $target_dir"
    ls -lh "$target_dir"
}

# ── 启动常驻开发容器 ──────────────────────────────────────────────────────
start_dev_container() {
    log_step "启动常驻开发容器 $CONTAINER_NAME"

    if docker ps --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        log_info "容器已在运行，跳过"
        return
    fi

    if docker ps -a --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        log_warn "容器已停止，正在重启..."
        docker rm -f "$CONTAINER_NAME"
    fi

    log_info "Starting container (ports: 3000→3000, 54320→54320, 54321→54321)..."
    # shellcheck disable=SC2086
    docker run -d \
        $DOCKER_PLATFORM_FLAG \
        --name "$CONTAINER_NAME" \
        -p 3000:3000 \
        -p 54320:54320 \
        -p 54321:54321 \
        -v "$TEACLAVE_SDK:/root/teaclave_sdk_src" \
        -v "$SHARED_DIR:/opt/teaclave/shared" \
        -v "$IMG_DIR:/opt/teaclave/images" \
        "$DOCKER_IMAGE" \
        tail -f /dev/null

    sleep 2
    log_info "Container started: $(docker ps --filter "name=$CONTAINER_NAME" --format "{{.Status}}")"
}

# ── 验证环境 ──────────────────────────────────────────────────────────────
verify_environment() {
    log_step "验证环境"
    docker exec "$CONTAINER_NAME" bash -l -c "
        echo '=== Rust toolchain ===' && rustup show active-toolchain 2>/dev/null || true
        echo '=== xargo ===' && which xargo 2>/dev/null && xargo --version 2>/dev/null || true
        echo '=== Cross compiler ===' && aarch64-linux-gnu-gcc --version | head -1 2>/dev/null || true
        echo '=== OP-TEE TA Dev Kit ===' && ls \$TA_DEV_KIT_DIR/scripts/ 2>/dev/null | head -5 || true
        echo '=== switch_config ===' && switch_config --status 2>/dev/null || true
    " || log_warn "Some verification steps failed (may be OK if container just started)"
}

# ── 汇总 ──────────────────────────────────────────────────────────────────
print_summary() {
    log_step "初始化完成"
    cat <<EOF

  ┌─────────────────────────────────────────────────────────┐
  │  AirAccount QEMU 开发环境已就绪                            │
  │                                                         │
  │  下一步：                                                 │
  │    make -C qemu build    # 构建 TA + CA                  │
  │    make -C qemu start    # 启动 QEMU (需要 tmux)          │
  │    make -C qemu deploy   # 部署到 QEMU guest              │
  │    make -C qemu test     # 运行集成测试                    │
  │                                                         │
  │  架构说明（Apple Silicon）：                               │
  │    macOS arm64 → OrbStack Rosetta → x86_64 容器          │
  │    → QEMU → aarch64 OP-TEE + Linux                       │
  │                                                         │
  │  共享目录（容器内可见）：                                   │
  │    $SHARED_DIR
  │                                                         │
  │  进入容器：                                               │
  │    docker exec -it $CONTAINER_NAME bash -l              │
  └─────────────────────────────────────────────────────────┘

EOF
}

# ── main ─────────────────────────────────────────────────────────────────
SKIP_EXTRACT=false
for arg in "$@"; do
    case "$arg" in
        --skip-extract) SKIP_EXTRACT=true ;;
        --help|-h)
            echo "Usage: $0 [--skip-extract]"
            echo "  --skip-extract  跳过 QEMU 镜像解包（镜像已在 /opt/teaclave/images/ 时使用）"
            exit 0 ;;
    esac
done

check_prerequisites
setup_directories
pull_docker_image
$SKIP_EXTRACT || extract_qemu_images
start_dev_container
verify_environment
print_summary
