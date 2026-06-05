#!/bin/bash
# Build AirAccount KMS for STM32MP157F-DK2 (ARMv7-A 32-bit)
# Cross-compiles TA + CA inside Docker, outputs to build/dk2/
#
# Prerequisites:
#   1. Docker running
#   2. TA Dev Kit: set DK2_TA_DEV_KIT_DIR to the export-ta_arm32 directory
#      obtained from ST SDK or a local OP-TEE build for STM32MP1.
#      See docs/dk2-deployment-guide.md § "Getting TA Dev Kit"
#
# Usage:
#   DK2_TA_DEV_KIT_DIR=/opt/STM32MP1-SDK/.../export-ta_arm32 ./scripts/dk2-build.sh
#   ./scripts/dk2-build.sh clean     # clean build

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_OUT="$PROJECT_ROOT/build/dk2"
TARGET="armv7-unknown-linux-gnueabihf"
CROSS="arm-linux-gnueabihf-"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"
IMAGE="stm32-builder"
CONTAINER="stm32-builder-run"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[dk2-build]${NC} $*"; }
ok()   { echo -e "${GREEN}[ok]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC} $*"; }
die()  { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

# --- clean ---
if [[ "${1:-}" == "clean" ]]; then
    log "Cleaning build/dk2/ ..."
    rm -rf "$BUILD_OUT"
    docker rm -f "$CONTAINER" 2>/dev/null || true
    ok "Clean done."
    exit 0
fi

# --- check TA Dev Kit ---
if [[ -z "${DK2_TA_DEV_KIT_DIR:-}" ]]; then
    die "DK2_TA_DEV_KIT_DIR is not set.\n  Set it to the export-ta_arm32 directory from your ST SDK or OP-TEE build.\n  See docs/dk2-deployment-guide.md for how to obtain it."
fi
[[ -d "$DK2_TA_DEV_KIT_DIR" ]] || die "DK2_TA_DEV_KIT_DIR=$DK2_TA_DEV_KIT_DIR does not exist."
ok "TA Dev Kit: $DK2_TA_DEV_KIT_DIR"

# --- build Docker image if needed ---
if ! docker image inspect "$IMAGE" &>/dev/null; then
    log "Building Docker image $IMAGE (first time, ~5 min)..."
    docker build --platform linux/amd64 \
        -f "$PROJECT_ROOT/docker/Dockerfile.stm32-builder" \
        -t "$IMAGE" "$PROJECT_ROOT"
fi

mkdir -p "$BUILD_OUT"

log "Cross-compiling TA (armv7)..."
docker run --rm --name "$CONTAINER" \
    --platform linux/amd64 \
    -v "$PROJECT_ROOT:/workspace" \
    -v "$DK2_TA_DEV_KIT_DIR:/ta-dev-kit:ro" \
    -e CROSS_COMPILE="$CROSS" \
    -e TARGET_TA="$TARGET" \
    -e TA_DEV_KIT_DIR="/ta-dev-kit" \
    "$IMAGE" bash -lc "
        set -euo pipefail
        cd /workspace/kms/ta
        make clean TARGET=$TARGET CROSS_COMPILE=$CROSS TA_DEV_KIT_DIR=/ta-dev-kit
        make TARGET=$TARGET CROSS_COMPILE=$CROSS TA_DEV_KIT_DIR=/ta-dev-kit
    "

log "Cross-compiling CA (armv7)..."
docker run --rm --name "$CONTAINER" \
    --platform linux/amd64 \
    -v "$PROJECT_ROOT:/workspace" \
    "$IMAGE" bash -lc "
        set -euo pipefail
        cd /workspace/kms/host
        cargo build --target $TARGET --release --bin kms-api-server 2>&1
    "

# --- collect artifacts ---
log "Collecting artifacts → build/dk2/"
TA_DIR="$PROJECT_ROOT/kms/ta/target/$TARGET/release"
CA_DIR="$PROJECT_ROOT/kms/host/target/$TARGET/release"

[[ -f "$TA_DIR/${UUID}.ta" ]] || die "TA artifact not found: $TA_DIR/${UUID}.ta"
[[ -f "$CA_DIR/kms-api-server" ]] || die "CA artifact not found: $CA_DIR/kms-api-server"

cp "$TA_DIR/${UUID}.ta"      "$BUILD_OUT/"
cp "$CA_DIR/kms-api-server"  "$BUILD_OUT/"

ok "Build complete. Artifacts in build/dk2/:"
ls -lh "$BUILD_OUT/"
echo ""
echo "  Next: DK2_BOARD_IP=192.168.7.2 ./scripts/dk2-deploy.sh"
