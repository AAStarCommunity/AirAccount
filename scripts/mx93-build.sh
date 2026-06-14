#!/bin/bash
# Build AirAccount KMS TA + CA for NXP FRDM-IMX93 (aarch64).
#
# Runs the cross-compile inside the teaclave_dev_env Docker container, then
# copies the signed TA and the CA binary into build/mx93/ where mx93-deploy.sh
# expects them.
#
# Usage:
#   ./scripts/mx93-build.sh [ta|ca|all]      # default: all
#
# See kms/docs/BUILD-MX93.md for the four cross-compile pitfalls this encodes:
#   1. release needs an explicit linker (check doesn't link, so it hides this)
#   2. C deps (secp256k1-sys) need CC_<target> or they compile to x86 (EM 62)
#   3. a failed build leaves the previous artifact behind — rm before building
#   4. container has no internet; CA needs Mac's ~/.cargo and stable (1.88),
#      because nightly-1.80 can't parse newer crate manifests (getrandom 0.4.2)

set -euo pipefail

MODE="${1:-all}"
CONTAINER="${TEACLAVE_CONTAINER:-teaclave_dev_env}"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_OUT="$PROJECT_ROOT/build/mx93"

# container-internal repo path (bind-mounted from host kms/)
C_KMS="/root/teaclave_sdk_src/projects/web3/kms"
TA_OUT_REL="ta/target/aarch64-unknown-optee/release"
CA_OUT_REL="host/target/aarch64-unknown-linux-gnu/release"
# host-side view of the same bind-mounted artifacts
H_TA_OUT="$PROJECT_ROOT/kms/$TA_OUT_REL"
H_CA_OUT="$PROJECT_ROOT/kms/$CA_OUT_REL"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
log()  { echo -e "${YELLOW}[mx93-build]${NC} $*"; }
ok()   { echo -e "${GREEN}[ok]${NC} $*"; }
die()  { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

docker ps --format '{{.Names}}' | grep -qx "$CONTAINER" \
    || die "Container '$CONTAINER' not running. Start it: docker start $CONTAINER (and OrbStack)."

# Ensure the MX93 TA signing key (NXP imx-optee-os lf-6.18 RSA-4096 public dev
# key) is present. It is gitignored (*.pem) and not committed, so fetch it from
# NXP's official repo on first build. MX93 OP-TEE 4.8 only trusts this 4096-bit
# key; the teaclave container default (2048-bit) makes OP-TEE reject the TA.
TA_SIGN_KEY="$PROJECT_ROOT/kms/keys/mx93_ta_sign_lf6.18.pem"
if [[ ! -f "$TA_SIGN_KEY" ]]; then
    log "MX93 TA signing key missing — downloading NXP lf-6.18 public dev key..."
    mkdir -p "$(dirname "$TA_SIGN_KEY")"
    curl -fsSL "https://raw.githubusercontent.com/nxp-imx/imx-optee-os/lf-6.18.2-1.0.0/keys/default.pem" \
        -o "$TA_SIGN_KEY" || die "Failed to download NXP TA signing key"
    ok "Downloaded MX93 TA signing key (NXP public dev key, RSA-4096)"
fi

build_ta() {
    # Issue #63: TA feature set. Default = transition (production-safe). Set
    # MX93_STRICT_CHALLENGE=1 to build a STRICT image (rejects assertions without
    # TA-issued challenge binding). Only build strict AFTER the SDK ships the
    # GetChallenge flow (#58) — otherwise every not-yet-migrated client is rejected.
    local TA_FEATURES="ree-fs-only"
    if [[ "${MX93_STRICT_CHALLENGE:-0}" == "1" ]]; then
        TA_FEATURES="ree-fs-only,strict-challenge"
        warn "STRICT challenge mode ON — legacy (no-clientDataJSON) clients will be REJECTED. Ensure SDK #58 is deployed."
    fi
    log "Building TA (aarch64-unknown-optee, nightly-2024-05-15, features: $TA_FEATURES)..."
    docker exec "$CONTAINER" bash -c '
      set -e
      export PATH=/root/.cargo/bin:$PATH
      export OPTEE_OS_DIR=/opt/teaclave/optee/optee_os
      export TA_DEV_KIT_DIR=$OPTEE_OS_DIR/out/arm-plat-vexpress/export-ta_arm64
      export TARGET_TA=aarch64-unknown-optee
      export CROSS_COMPILE_TA=aarch64-linux-gnu-
      export RUST_TARGET_PATH=/opt/teaclave/std
      export RUSTUP_TOOLCHAIN=nightly-2024-05-15
      export CARGO_NET_OFFLINE=true
      export CARGO_TARGET_AARCH64_UNKNOWN_OPTEE_LINKER=aarch64-linux-gnu-gcc
      export CC_aarch64_unknown_optee=aarch64-linux-gnu-gcc
      export AR_aarch64_unknown_optee=aarch64-linux-gnu-ar
      export HOST_CC=gcc
      unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY
      cd '"$C_KMS"'/ta
      UUID='"$UUID"'
      OUT=target/aarch64-unknown-optee/release
      rm -f $OUT/ta $OUT/stripped_ta $OUT/$UUID.ta
      # ree-fs-only: MX93 eMMC RPMB authentication key is NOT programmed
      # (issue #50). The TA must use REE-FS storage and never issue an RPMB
      # syscall, otherwise RPMB access faults and kills the TA. Remove this
      # feature once the RPMB key is programmed in production to enable
      # hardware anti-rollback.
      xargo build --release --target aarch64-unknown-optee --features '"$TA_FEATURES"'
      file $OUT/ta | grep -q "ARM aarch64" || { echo "TA is not aarch64!"; exit 1; }
      aarch64-linux-gnu-objcopy --strip-unneeded $OUT/ta $OUT/stripped_ta
      # MX93 OP-TEE 4.8 (NXP LF 6.18) trusts an RSA-4096 TA signing key, NOT the
      # teaclave container default (RSA-2048). Signing with 2048 makes OP-TEE
      # reject the TA at load time with security fault 0xffff000f. This key is
      # NXP imx-optee-os lf-6.18.2-1.0.0 keys/default.pem (public dev key).
      python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
        --uuid $UUID --ta-version 0 \
        --in $OUT/stripped_ta --out $OUT/$UUID.ta \
        --key /root/teaclave_sdk_src/projects/web3/kms/keys/mx93_ta_sign_lf6.18.pem
    ' || die "TA build failed"
    [[ -f "$H_TA_OUT/$UUID.ta" ]] || die "Signed TA not found at $H_TA_OUT/$UUID.ta"
    mkdir -p "$BUILD_OUT"
    cp "$H_TA_OUT/$UUID.ta" "$BUILD_OUT/$UUID.ta"
    ok "TA -> $BUILD_OUT/$UUID.ta ($(du -h "$BUILD_OUT/$UUID.ta" | cut -f1))"
}

build_ca() {
    log "Building CA (aarch64-unknown-linux-gnu, stable 1.88)..."
    docker exec "$CONTAINER" bash -c '
      set -e
      export PATH=/root/.cargo/bin:$PATH
      export RUSTUP_TOOLCHAIN=stable
      export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64
      export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
      export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
      export AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar
      export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
      export HOST_CC=gcc
      export CARGO_NET_OFFLINE=true
      unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY
      cd '"$C_KMS"'/host
      rm -f target/aarch64-unknown-linux-gnu/release/kms-api-server
      # NOTE: release build = NO features. Decentralized KMS has no admin surface,
      # so /admin/purge-key (the "admin-purge" compile-time feature) is NOT built in.
      # For a beta/test image that needs admin force-delete, add: --features admin-purge
      # (still requires KMS_ADMIN_TOKEN at runtime). See kms/docs/RELEASE-PLAN.md.
      cargo build --release --target aarch64-unknown-linux-gnu --bin kms-api-server
      file target/aarch64-unknown-linux-gnu/release/kms-api-server | grep -q "ARM aarch64" \
        || { echo "CA is not aarch64!"; exit 1; }
    ' || die "CA build failed (see kms/docs/BUILD-MX93.md pitfall 4 if it is a dependency/manifest error)"
    [[ -f "$H_CA_OUT/kms-api-server" ]] || die "CA binary not found at $H_CA_OUT/kms-api-server"
    mkdir -p "$BUILD_OUT"
    cp "$H_CA_OUT/kms-api-server" "$BUILD_OUT/kms-api-server"
    ok "CA -> $BUILD_OUT/kms-api-server ($(du -h "$BUILD_OUT/kms-api-server" | cut -f1))"
}

case "$MODE" in
    ta)  build_ta ;;
    ca)  build_ca ;;
    all) build_ta; build_ca ;;
    *)   die "Unknown mode '$MODE' (use: ta | ca | all)" ;;
esac

ok "Build complete. Deploy with: MX93_BOARD_IP=192.168.2.30 ./scripts/mx93-deploy.sh"
