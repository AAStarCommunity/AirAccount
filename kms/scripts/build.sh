#!/bin/bash
# Step 1: Build TA + CA in Docker
# Usage: ./build.sh [ta|ca|all]
# Default: all

set -eo pipefail

MODE="${1:-all}"
YELLOW='\033[1;33m'; GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'

echo "${YELLOW}[Step 1] Build KMS (mode: $MODE)${NC}"
echo ""

BUILD_CMD='
source /root/.cargo/env

# ===== Environment =====
export TA_DEV_KIT_DIR=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/usr/include/optee/export-user_ta
export RUST_TARGET_PATH=/workspace/sdks/rust-sdk
export CC_arm_unknown_optee=/tmp/arm-wrapper-gcc
export TARGET_TA=arm-unknown-optee
export OPTEE_CLIENT_EXPORT=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/tmp/arm-ca-gcc
export CC=/tmp/arm-ca-gcc
CROSS_STRIP=/opt/st/stm32mp1/6.6-v26.02.18/sysroots/aarch64-ostl_sdk-linux/usr/bin/arm-ostl-linux-gnueabi/arm-ostl-linux-gnueabi-strip
TA_UUID=4319f351-0b24-4097-b659-80ee4f824cdd
'

if [ "$MODE" = "ta" ] || [ "$MODE" = "all" ]; then
    echo "${YELLOW}>>> Building TA...${NC}"
    docker exec stm32-builder bash -c "$BUILD_CMD"'
cd /workspace/kms/ta
HOST_CC=gcc xargo build --target arm-unknown-optee --release \
  --config "target.arm-unknown-optee.linker=\"/tmp/arm-wrapper-gcc\"" || exit 1

# Post-build: strip + fix + sign
TARGET_DIR=target/arm-unknown-optee/release
cp $TARGET_DIR/ta $TARGET_DIR/ta.stripped
$CROSS_STRIP $TARGET_DIR/ta.stripped
python3 fix_ta_elf.py $TARGET_DIR/ta.stripped $TARGET_DIR/ta.fixed
python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
  --uuid $TA_UUID --ta-version 0 \
  --in $TARGET_DIR/ta.fixed \
  --out $TARGET_DIR/${TA_UUID}.ta \
  --key $TA_DEV_KIT_DIR/keys/default_ta.pem || exit 1
echo ">>> TA: $TARGET_DIR/${TA_UUID}.ta"
'
    echo "${GREEN}TA build done${NC}"
fi

if [ "$MODE" = "ca" ] || [ "$MODE" = "all" ]; then
    echo "${YELLOW}>>> Building CA...${NC}"
    docker exec stm32-builder bash -c "$BUILD_CMD"'
cd /workspace/kms/host
cargo build --target armv7-unknown-linux-gnueabihf --release --bin kms-api-server || exit 1
cargo build --target armv7-unknown-linux-gnueabihf --release --bin api-key || exit 1
echo ">>> CA: target/armv7-unknown-linux-gnueabihf/release/kms-api-server"
'
    echo "${GREEN}CA build done${NC}"
fi

echo ""
echo "${GREEN}[Step 1] Build complete${NC}"
