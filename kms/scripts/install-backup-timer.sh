#!/usr/bin/env bash
# =============================================================================
# install-backup-timer.sh — Install KMS backup as a systemd timer (hourly)
#
# This script installs:
#   /etc/systemd/system/kms-backup.service  — runs backup.sh
#   /etc/systemd/system/kms-backup.timer    — fires OnBootSec=5min, then hourly
#
# Usage:
#   install-backup-timer.sh [OPTIONS]
#
# Options:
#   --remote user@host:/path   Configure remote push target in the service
#   --dest /path               Override local backup root (default: /root/backups/kms)
#   --uninstall                Stop + disable + remove the timer and service units
#   --status                   Show current timer and service status
#   -h, --help                 Show this help
#
# After installation:
#   - Backup runs 5 minutes after boot, then every hour
#   - Logs go to /var/log/kms-backup.log AND journald (journalctl -u kms-backup)
#   - To trigger a manual run: systemctl start kms-backup.service
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKUP_SCRIPT="${SCRIPT_DIR}/backup.sh"
SERVICE_FILE="/etc/systemd/system/kms-backup.service"
TIMER_FILE="/etc/systemd/system/kms-backup.timer"

REMOTE_ARG=""
DEST_ARG=""
UNINSTALL=false
STATUS_ONLY=false

# ─── Argument parsing ─────────────────────────────────────────────────────────
usage() {
    grep '^#' "$0" | grep -v '^#!/' | sed 's/^# \{0,2\}//' | head -30
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --remote)     REMOTE_ARG="$2"; shift ;;
        --remote=*)   REMOTE_ARG="${1#*=}" ;;
        --dest)       DEST_ARG="$2"; shift ;;
        --dest=*)     DEST_ARG="${1#*=}" ;;
        --uninstall)  UNINSTALL=true ;;
        --status)     STATUS_ONLY=true ;;
        -h|--help)    usage ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
    shift
done

# ─── Root check ───────────────────────────────────────────────────────────────
if [[ "$EUID" -ne 0 ]]; then
    echo "ERROR: This script must be run as root (or with sudo)." >&2
    exit 1
fi

# ─── Status mode ──────────────────────────────────────────────────────────────
if [[ "$STATUS_ONLY" == true ]]; then
    echo "=== kms-backup.timer ==="
    systemctl status kms-backup.timer --no-pager 2>/dev/null || echo "(not installed)"
    echo ""
    echo "=== kms-backup.service ==="
    systemctl status kms-backup.service --no-pager 2>/dev/null || echo "(not installed)"
    echo ""
    echo "=== Next scheduled run ==="
    systemctl list-timers kms-backup.timer --no-pager 2>/dev/null || echo "(not scheduled)"
    exit 0
fi

# ─── Uninstall mode ───────────────────────────────────────────────────────────
if [[ "$UNINSTALL" == true ]]; then
    echo "Uninstalling KMS backup timer..."

    systemctl stop kms-backup.timer 2>/dev/null && echo "Stopped kms-backup.timer" || true
    systemctl stop kms-backup.service 2>/dev/null || true
    systemctl disable kms-backup.timer 2>/dev/null && echo "Disabled kms-backup.timer" || true

    [[ -f "$TIMER_FILE" ]] && rm -f "$TIMER_FILE" && echo "Removed ${TIMER_FILE}"
    [[ -f "$SERVICE_FILE" ]] && rm -f "$SERVICE_FILE" && echo "Removed ${SERVICE_FILE}"

    systemctl daemon-reload
    echo "Uninstall complete."
    exit 0
fi

# ─── Verify backup.sh is present ──────────────────────────────────────────────
if [[ ! -f "$BACKUP_SCRIPT" ]]; then
    echo "ERROR: backup.sh not found at: ${BACKUP_SCRIPT}" >&2
    echo "Make sure you run this script from the kms/scripts/ directory," >&2
    echo "or that backup.sh exists alongside install-backup-timer.sh." >&2
    exit 1
fi

chmod +x "$BACKUP_SCRIPT"

# ─── Build ExecStart command ──────────────────────────────────────────────────
EXEC_START="${BACKUP_SCRIPT}"
[[ -n "$DEST_ARG" ]]   && EXEC_START+=" --dest '${DEST_ARG}'"
[[ -n "$REMOTE_ARG" ]] && EXEC_START+=" --remote '${REMOTE_ARG}'"

echo "Installing KMS backup systemd timer..."
echo "  backup.sh location : ${BACKUP_SCRIPT}"
[[ -n "$DEST_ARG" ]]   && echo "  local dest override: ${DEST_ARG}"
[[ -n "$REMOTE_ARG" ]] && echo "  remote push target : ${REMOTE_ARG}"
echo ""

# ─── Write kms-backup.service ─────────────────────────────────────────────────
cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=KMS metadata backup (CA binary, TA binary, kms.db, config)
Documentation=file://${SCRIPT_DIR}/backup.sh
# Run after network is up so remote push can succeed
After=network-online.target
Wants=network-online.target
# Do not block boot if backup fails
DefaultDependencies=no

[Service]
Type=oneshot
# Run as root so we can read all KMS files
User=root
Group=root

ExecStart=${EXEC_START}

# Give the backup up to 30 minutes to complete (large first-run or slow SSH push)
TimeoutStartSec=1800

# Log to journald (in addition to the log file backup.sh writes)
StandardOutput=journal
StandardError=journal
SyslogIdentifier=kms-backup

# Restart policy: do not restart on failure (timer will retry next hour)
Restart=no

[Install]
WantedBy=multi-user.target
EOF

echo "Written: ${SERVICE_FILE}"

# ─── Write kms-backup.timer ───────────────────────────────────────────────────
cat > "$TIMER_FILE" <<EOF
[Unit]
Description=Hourly KMS metadata backup timer
Documentation=file://${SCRIPT_DIR}/install-backup-timer.sh

[Timer]
# First run 5 minutes after boot (gives services time to stabilize)
OnBootSec=5min
# Then run every hour
OnUnitActiveSec=1h
# If the system was off when a scheduled run was missed, run immediately on next boot
Persistent=true
# Which service unit to activate
Unit=kms-backup.service

[Install]
WantedBy=timers.target
EOF

echo "Written: ${TIMER_FILE}"

# ─── Enable and start ─────────────────────────────────────────────────────────
systemctl daemon-reload
echo "daemon-reload done"

systemctl enable kms-backup.timer
echo "Enabled kms-backup.timer (will survive reboots)"

systemctl start kms-backup.timer
echo "Started kms-backup.timer"

# ─── Verify ───────────────────────────────────────────────────────────────────
echo ""
echo "=== Timer status ==="
systemctl status kms-backup.timer --no-pager || true

echo ""
echo "=== Next scheduled runs ==="
systemctl list-timers kms-backup.timer --no-pager || true

echo ""
echo "Installation complete."
echo ""
echo "Useful commands:"
echo "  Trigger backup now   : systemctl start kms-backup.service"
echo "  View timer status    : systemctl status kms-backup.timer"
echo "  View last run logs   : journalctl -u kms-backup --no-pager -n 50"
echo "  View backup log file : tail -f /var/log/kms-backup.log"
echo "  List backups         : ${SCRIPT_DIR}/backup-restore.sh --list"
echo "  Uninstall timer      : $0 --uninstall"
