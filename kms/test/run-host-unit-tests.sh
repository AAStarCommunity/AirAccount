#!/bin/bash
# Cross-compile the host (CA) unit tests for aarch64 and run them ON THE BOARD.
#
# Why not just `cargo test` on the Mac?  The host crate links optee-teec, whose
# build.rs needs OPTEE_CLIENT_EXPORT and a real libteec.  Only ARM32/ARM64 libteec
# exist in the teaclave_dev_env container (no x86_64), so the test binary must be
# cross-compiled for aarch64-unknown-linux-gnu (same target as the deployed CA)
# and executed on the i.MX93 board.
#
# Usage:  ./kms/test/run-host-unit-tests.sh [board_ip]
#
# proto unit tests, by contrast, are pure and run anywhere:
#   cargo test --manifest-path kms/proto/Cargo.toml

set -euo pipefail
BOARD_IP="${1:-192.168.2.30}"
CONTAINER="${TEACLAVE_CONTAINER:-teaclave_dev_env}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
C_KMS="/root/teaclave_sdk_src/projects/web3/kms"

echo "[1/3] Cross-compiling host unit tests (aarch64, in $CONTAINER)…"
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
  cargo test --lib --release --target aarch64-unknown-linux-gnu --no-run 2>&1 | tail -3
'

TB=$(ls -t "$ROOT"/kms/host/target/aarch64-unknown-linux-gnu/release/deps/kms-* 2>/dev/null \
       | grep -vE "\.(d|rlib)$" | head -1)
[ -n "$TB" ] || { echo "test binary not found"; exit 1; }
file "$TB" | grep -q "ARM aarch64" || { echo "test binary is not aarch64!"; exit 1; }

echo "[2/3] Copying test binary → board…"
scp -o BatchMode=yes "$TB" "root@$BOARD_IP:/tmp/kms-host-tests" >/dev/null

echo "[3/3] Running on board:"
ssh -o BatchMode=yes "root@$BOARD_IP" 'chmod +x /tmp/kms-host-tests && /tmp/kms-host-tests'
