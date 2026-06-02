#!/usr/bin/env bash
# qemu/build.sh — 构建 TA (Trusted Application) + CA (kms-api-server)
#
# 用法：
#   ./qemu/build.sh          # 构建 TA + CA
#   ./qemu/build.sh ta       # 仅构建 TA
#   ./qemu/build.sh ca       # 仅构建 CA
#   ./qemu/build.sh clean    # 清理构建产物

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/qemu/lib/log.sh"

CONTAINER_NAME="teaclave_dev_env"
TA_UUID="4319f351-0b24-4097-b659-80ee4f824cdd"
KMS_PROJECT="/root/teaclave_sdk_src/projects/web3/kms"
SHARED_DIR="/opt/teaclave/shared"

check_container() {
    if ! docker ps --format "{{.Names}}" | grep -q "^${CONTAINER_NAME}$"; then
        log_error "开发容器未运行！先执行: ./qemu/setup.sh"
        exit 1
    fi
}

build_ta() {
    log_step "构建 TA (aarch64-unknown-optee)"
    docker exec "$CONTAINER_NAME" bash -l -c "
        set -e
        cd ${KMS_PROJECT}/ta
        echo '--- Rust target ---'
        rustup target list --installed | grep optee || true

        CC=aarch64-linux-gnu-gcc \
        xargo build --target aarch64-unknown-optee --release 2>&1

        echo '--- 签名 TA ---'
        aarch64-linux-gnu-objcopy --strip-unneeded \
            target/aarch64-unknown-optee/release/ta \
            target/aarch64-unknown-optee/release/stripped_ta

        python3 \$TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
            --uuid ${TA_UUID} \
            --ta-version 1 \
            --in  target/aarch64-unknown-optee/release/stripped_ta \
            --out target/aarch64-unknown-optee/release/${TA_UUID}.ta \
            --key \$TA_DEV_KIT_DIR/keys/default_ta.pem

        echo '--- 部署 TA 到 shared ---'
        mkdir -p ${SHARED_DIR}/ta
        cp target/aarch64-unknown-optee/release/${TA_UUID}.ta ${SHARED_DIR}/ta/
        ls -lh ${SHARED_DIR}/ta/
    "
    log_info "TA 构建完成：${TA_UUID}.ta"
}

build_ca() {
    log_step "构建 CA (aarch64-unknown-linux-gnu)"
    docker exec "$CONTAINER_NAME" bash -l -c "
        set -e
        cd ${KMS_PROJECT}/host
        cargo build --target aarch64-unknown-linux-gnu --release --bin kms-api-server 2>&1

        echo '--- 部署 CA 到 shared ---'
        cp target/aarch64-unknown-linux-gnu/release/kms-api-server ${SHARED_DIR}/
        ls -lh ${SHARED_DIR}/kms-api-server
    "
    log_info "CA 构建完成：kms-api-server"
}

clean_build() {
    log_step "清理构建产物"
    docker exec "$CONTAINER_NAME" bash -l -c "
        cd ${KMS_PROJECT}/ta   && cargo clean 2>/dev/null || true
        cd ${KMS_PROJECT}/host && cargo clean 2>/dev/null || true
        rm -f ${SHARED_DIR}/kms-api-server ${SHARED_DIR}/ta/${TA_UUID}.ta
    "
    log_info "清理完成"
}

check_container

case "${1:-all}" in
    ta)    build_ta ;;
    ca)    build_ca ;;
    clean) clean_build ;;
    all)
        build_ta
        build_ca
        log_step "构建汇总"
        docker exec "$CONTAINER_NAME" ls -lh "${SHARED_DIR}/" "${SHARED_DIR}/ta/" 2>/dev/null || true
        log_info "全部构建完成 ✓  下一步: make -C qemu start"
        ;;
    *)
        echo "用法: $0 [ta|ca|all|clean]"
        exit 1 ;;
esac
