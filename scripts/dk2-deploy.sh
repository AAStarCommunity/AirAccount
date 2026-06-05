#!/bin/bash
# Deploy AirAccount KMS to STM32MP157F-DK2
# Copies TA + CA artifacts from build/dk2/ to the board, then restarts services.
#
# Usage:
#   DK2_BOARD_IP=192.168.7.2 ./scripts/dk2-deploy.sh
#   DK2_BOARD_IP=192.168.7.2 DK2_BOARD_USER=root ./scripts/dk2-deploy.sh
#   DK2_BOARD_IP=192.168.7.2 ./scripts/dk2-deploy.sh --first-run   # installs systemd service

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_OUT="$PROJECT_ROOT/build/dk2"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

BOARD_IP="${DK2_BOARD_IP:-192.168.7.2}"
BOARD_USER="${DK2_BOARD_USER:-root}"
FIRST_RUN=false
[[ "${1:-}" == "--first-run" ]] && FIRST_RUN=true

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'
log()  { echo -e "${BLUE}[dk2-deploy]${NC} $*"; }
ok()   { echo -e "${GREEN}[ok]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC} $*"; }
die()  { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

board() { ssh "${BOARD_USER}@${BOARD_IP}" "$@"; }
push()  { scp "$1" "${BOARD_USER}@${BOARD_IP}:$2"; }

# --- check artifacts ---
[[ -f "$BUILD_OUT/${UUID}.ta" ]]     || die "TA not found. Run ./scripts/dk2-build.sh first."
[[ -f "$BUILD_OUT/kms-api-server" ]] || die "CA not found. Run ./scripts/dk2-build.sh first."

log "Target board: ${BOARD_USER}@${BOARD_IP}"
log "TA:  $BUILD_OUT/${UUID}.ta  ($(du -h "$BUILD_OUT/${UUID}.ta" | cut -f1))"
log "CA:  $BUILD_OUT/kms-api-server  ($(du -h "$BUILD_OUT/kms-api-server" | cut -f1))"

# --- connectivity check ---
ssh -o ConnectTimeout=5 -o BatchMode=yes "${BOARD_USER}@${BOARD_IP}" true \
    || die "Cannot SSH to ${BOARD_USER}@${BOARD_IP}. Check:\n  - Board powered and connected\n  - IP correct (DK2_BOARD_IP env var)\n  - SSH key added (ssh-copy-id root@${BOARD_IP})"

ok "Board reachable."

# --- stop service if running ---
board "systemctl stop kms-api-server 2>/dev/null || pkill kms-api-server 2>/dev/null || true"

# --- deploy TA ---
log "Deploying TA → /lib/optee_armtz/"
board "mkdir -p /lib/optee_armtz"
push "$BUILD_OUT/${UUID}.ta" "/lib/optee_armtz/${UUID}.ta"
board "chmod 444 /lib/optee_armtz/${UUID}.ta"
ok "TA deployed."

# --- deploy CA ---
log "Deploying CA → /usr/local/bin/kms-api-server"
push "$BUILD_OUT/kms-api-server" "/usr/local/bin/kms-api-server"
board "chmod +x /usr/local/bin/kms-api-server"
ok "CA deployed."

# --- install systemd service (first run only) ---
if $FIRST_RUN; then
    log "Installing systemd service..."
    board "mkdir -p /data/kms"
    board "cat > /etc/systemd/system/kms-api-server.service" <<'EOF'
[Unit]
Description=AirAccount KMS API Server
After=network.target tee-supplicant.service
Requires=tee-supplicant.service

[Service]
Type=simple
ExecStart=/usr/local/bin/kms-api-server
Restart=always
RestartSec=5
User=root
Environment=KMS_DB_PATH=/data/kms/kms.db
Environment=KMS_ORIGIN=https://kms.aastar.io
WorkingDirectory=/data/kms
StandardOutput=journal
StandardError=journal
NoNewPrivileges=yes

[Install]
WantedBy=multi-user.target
EOF
    board "systemctl daemon-reload && systemctl enable kms-api-server"
    ok "systemd service installed and enabled."
fi

# --- reload TA (restart tee-supplicant) ---
log "Reloading TA (restart tee-supplicant)..."
board "systemctl restart tee-supplicant"
sleep 2

# --- start service ---
log "Starting kms-api-server..."
board "systemctl start kms-api-server"
sleep 3

# --- smoke test ---
log "Smoke test: GET /health"
HEALTH=$(ssh "${BOARD_USER}@${BOARD_IP}" "curl -sf http://127.0.0.1:3000/health 2>/dev/null || echo FAIL")
if [[ "$HEALTH" == "FAIL" ]]; then
    warn "Health check failed. Showing logs:"
    board "journalctl -u kms-api-server -n 30 --no-pager" || true
    die "Deploy failed — service not responding on :3000"
fi

ok "Deploy successful!"
echo ""
echo "  Health: $HEALTH"
echo "  Logs:   ssh ${BOARD_USER}@${BOARD_IP} 'journalctl -u kms-api-server -f'"
echo "  TEE:    ssh ${BOARD_USER}@${BOARD_IP} 'cat /sys/kernel/debug/optee/call_count'"
