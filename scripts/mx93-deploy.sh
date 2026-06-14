#!/bin/bash
# Deploy AirAccount KMS to NXP FRDM-IMX93 (aarch64)
# Copies TA + CA artifacts from build/mx93/ to the board, then restarts services.
#
# Usage:
#   MX93_BOARD_IP=<ip> ./scripts/mx93-deploy.sh
#   MX93_BOARD_IP=<ip> MX93_BOARD_USER=root ./scripts/mx93-deploy.sh
#   MX93_BOARD_IP=<ip> ./scripts/mx93-deploy.sh --first-run   # installs systemd service
#
# Environment variables:
#   MX93_BOARD_IP    board IP address (required)
#   MX93_BOARD_USER  SSH user (default: root)
#
# Note: The deployed service is named kms-api.service (not kms-api-server.service).
# dirf-repair.service handles the 0-byte/corrupt dirf.db bug that causes
# TEE_ERROR_CORRUPT_OBJECT (0xffff3025 / 0xf0100001) after eMMC re-flash.
# It runs as a separate oneshot unit (not ExecStartPre) to avoid the systemd
# deadlock that occurs when restarting a Requires= dependency from within the
# dependent unit's own startup.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_OUT="$PROJECT_ROOT/build/mx93"
SERVICE_FILE="$PROJECT_ROOT/kms/deploy/mx93/kms-api.service"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

BOARD_IP="${MX93_BOARD_IP:-}"
BOARD_USER="${MX93_BOARD_USER:-root}"
FIRST_RUN=false
[[ "${1:-}" == "--first-run" ]] && FIRST_RUN=true

REPAIR_SERVICE_FILE="$PROJECT_ROOT/kms/deploy/mx93/dirf-repair.service"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[mx93-deploy]${NC} $*"; }
ok()   { echo -e "${GREEN}[ok]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC} $*"; }
die()  { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

board() { ssh "${BOARD_USER}@${BOARD_IP}" "$@"; }
push()  { scp "$1" "${BOARD_USER}@${BOARD_IP}:$2"; }

# --- validate required env ---
[[ -n "$BOARD_IP" ]] || die "MX93_BOARD_IP is not set. Example: MX93_BOARD_IP=192.168.1.100 $0"

# --- check artifacts ---
[[ -f "$BUILD_OUT/${UUID}.ta" ]]     || die "TA not found at $BUILD_OUT/${UUID}.ta. Run ./scripts/mx93-build.sh first."
[[ -f "$BUILD_OUT/kms-api-server" ]] || die "CA not found at $BUILD_OUT/kms-api-server. Run ./scripts/mx93-build.sh first."
[[ -f "$SERVICE_FILE" ]]             || die "Service file not found at $SERVICE_FILE."
[[ -f "$REPAIR_SERVICE_FILE" ]]      || die "Service file not found at $REPAIR_SERVICE_FILE."

log "Target board: ${BOARD_USER}@${BOARD_IP} (NXP FRDM-IMX93, aarch64)"
log "TA:  $BUILD_OUT/${UUID}.ta  ($(du -h "$BUILD_OUT/${UUID}.ta" | cut -f1))"
log "CA:  $BUILD_OUT/kms-api-server  ($(du -h "$BUILD_OUT/kms-api-server" | cut -f1))"

# --- connectivity check ---
ssh -o ConnectTimeout=5 -o BatchMode=yes "${BOARD_USER}@${BOARD_IP}" true \
    || die "Cannot SSH to ${BOARD_USER}@${BOARD_IP}. Check:\n  - Board powered and connected via USB-C / Ethernet\n  - IP correct (MX93_BOARD_IP env var)\n  - SSH key added (ssh-copy-id root@${BOARD_IP})"

ok "Board reachable."

# --- stop service if running ---
log "Stopping kms-api.service..."
board "systemctl stop kms-api.service 2>/dev/null || pkill kms-api-server 2>/dev/null || true"

# --- deploy TA ---
log "Deploying TA → /lib/optee_armtz/"
board "mkdir -p /lib/optee_armtz"
push "$BUILD_OUT/${UUID}.ta" "/lib/optee_armtz/${UUID}.ta"
board "chmod 444 /lib/optee_armtz/${UUID}.ta"
ok "TA deployed."

# --- deploy CA ---
log "Deploying CA → /root/AirAccount/target/release/kms-api-server"
board "mkdir -p /root/AirAccount/target/release"
push "$BUILD_OUT/kms-api-server" "/root/AirAccount/target/release/kms-api-server"
board "chmod +x /root/AirAccount/target/release/kms-api-server"
ok "CA deployed."

# --- install/update systemd services ---
log "Installing/updating systemd services..."
push "$SERVICE_FILE" "/etc/systemd/system/kms-api.service"
push "$REPAIR_SERVICE_FILE" "/etc/systemd/system/dirf-repair.service"
board "systemctl daemon-reload"
if $FIRST_RUN; then
    board "systemctl enable dirf-repair.service"
    board "systemctl enable kms-api.service"
    ok "Services enabled (will start on boot)."
fi
ok "Service files updated."

# --- reload TA (restart tee-supplicant on board, WAIT until it is actually
#     ready before starting kms-api) ---
# Root cause of the old flaky deploy: kms-api's TEE worker opens its TA session
# at startup. If tee-supplicant is not ready yet, that open_session panics and
# the worker thread dies — but the warp HTTP server stays up, so the service is
# "active" and /health still passes, masking a dead TEE. We therefore (a) wait
# for tee-supplicant to reach active state (not a blind sleep) plus a short
# settle delay, and (b) smoke-test a TA-touching endpoint and restart kms-api on
# failure (the retry succeeds once supplicant is ready).
log "Reloading TA (restarting tee-supplicant@teepriv0.service)..."
board "systemctl restart tee-supplicant@teepriv0.service"
board "for i in \$(seq 1 15); do systemctl is-active tee-supplicant@teepriv0.service >/dev/null 2>&1 && exit 0; sleep 1; done; exit 1" \
    || warn "tee-supplicant did not report active within 15s — continuing, kms-api retry will cover it"
# Settle delay: supplicant 'active' can still briefly precede readiness.
board "sleep 3"

# --- start services (dirf-repair runs first, then kms-api) ---
log "Starting kms-api.service (dirf-repair.service will run first)..."
board "systemctl start kms-api.service"
# Wait on the board for the service to reach active state (max 15s)
board "for i in \$(seq 1 15); do systemctl is-active kms-api.service >/dev/null 2>&1 && exit 0; sleep 1; done; exit 1" \
    || { warn "Service did not become active within 15s"; board "journalctl -u kms-api.service -n 20 --no-pager 2>/dev/null || tail -20 /var/log/kms-api.log" || true; die "Deploy failed"; }

# --- HTTP smoke test (warp up?) ---
log "Smoke test: GET /health"
HEALTH=$(board "curl -sf http://127.0.0.1:3000/health 2>/dev/null || echo FAIL")
if [[ "$HEALTH" == "FAIL" ]]; then
    warn "Health check failed. Showing logs:"
    board "journalctl -u kms-api.service -n 30 --no-pager 2>/dev/null || tail -30 /var/log/kms-api.log" || true
    die "Deploy failed — service not responding on :3000"
fi

# --- TEE-worker smoke test (TA session alive?) with retry ---
# /health does NOT touch the TA, so it cannot detect a dead worker. /RollbackCounter
# does. If the worker died on the startup race, "TEE worker thread has exited"
# comes back — restart kms-api and re-check (up to 3 attempts, 3s apart).
log "Smoke test: GET /RollbackCounter (verifies TEE worker session is alive)"
TEE_OK=false
for attempt in 1 2 3; do
    RESP=$(board "curl -s http://127.0.0.1:3000/RollbackCounter 2>/dev/null || echo FAIL")
    if [[ "$RESP" == *'"counter"'* ]]; then
        TEE_OK=true
        break
    fi
    warn "TEE worker not ready (attempt $attempt/3): ${RESP:-<empty>} — restarting kms-api in 3s..."
    board "sleep 3 && systemctl restart kms-api.service"
    board "for i in \$(seq 1 15); do systemctl is-active kms-api.service >/dev/null 2>&1 && exit 0; sleep 1; done; exit 1" || true
done
if [[ "$TEE_OK" != true ]]; then
    warn "TEE worker still not alive after 3 attempts. Showing logs:"
    board "journalctl -u kms-api.service -n 30 --no-pager 2>/dev/null || tail -30 /var/log/kms-api.log" || true
    die "Deploy failed — TEE worker session not established (TA commands would all fail)"
fi
ok "TEE worker alive."

ok "Deploy successful!"
echo ""
echo "  Health: $HEALTH"
echo "  Logs:   ssh ${BOARD_USER}@${BOARD_IP} 'tail -f /var/log/kms-api.log'"
echo "  TEE:    ssh ${BOARD_USER}@${BOARD_IP} 'cat /sys/kernel/debug/optee/call_count'"
echo "  Status: ssh ${BOARD_USER}@${BOARD_IP} 'systemctl status kms-api.service'"
