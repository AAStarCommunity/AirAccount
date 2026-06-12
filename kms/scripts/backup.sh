#!/usr/bin/env bash
# =============================================================================
# backup.sh — KMS CA/TA metadata incremental backup
#
# What is backed up:
#   - kms.db          (SQLite wallet metadata: addresses, IDs — NO private keys)
#   - TA binary       (/lib/optee_armtz/<uuid>.ta)
#   - CA binary       (kms-api-server release build)
#   - systemd service config
#   - cloudflared tunnel config (if present)
#   - deployment scripts (kms/scripts/)
#   - system info snapshot (kernel, OP-TEE version, service status)
#
# What is NOT backed up (and why):
#   - /var/lib/tee/   — TEE secure storage; encrypted + hardware-bound; useless
#                       without the exact same physical TEE. Attempting to restore
#                       these to a different board would yield nothing.
#   - /dev/mmcblk0rpmb — RPMB partition; hardware-bound, cannot be read as a file
#   - *.key *.pem *.priv — any accidentally present private key material
#
# Usage:
#   backup.sh [OPTIONS]
#
# Options:
#   --full                Force full backup (ignore previous backup for link-dest)
#   --dest /path          Override local backup destination root
#                         (default: /root/backups/kms)
#   --remote user@host:/path  After local backup, rsync to remote over SSH
#   --dry-run             Print what would be done without writing anything
#   -h, --help            Show this help
#
# Storage layout:
#   /root/backups/kms/
#     YYYY-MM-DD_HHMMSS/      <- each backup timestamped
#       files/                <- actual backed-up files (rsync --link-dest)
#       manifest.sha256       <- SHA-256 of every file in files/
#       backup-info.txt       <- metadata (type, duration, sizes, system info)
#     latest -> YYYY-MM-DD_HHMMSS   <- symlink to newest
#
# Rotation (run automatically at end of backup):
#   Keep last 30 daily, last 12 monthly, last 5 yearly
#
# Logging:
#   All output mirrored to /var/log/kms-backup.log (appended)
# =============================================================================

set -euo pipefail

# ─── Constants ────────────────────────────────────────────────────────────────
BACKUP_ROOT="/root/backups/kms"
LOG_FILE="/var/log/kms-backup.log"
TA_UUID="4319f351-0b24-4097-b659-80ee4f824cdd"
TA_PATH="/lib/optee_armtz/${TA_UUID}.ta"
CA_PATH="/root/AirAccount/target/release/kms-api-server"
DB_PATH="/root/AirAccount/kms.db"
SERVICE_PATH="/etc/systemd/system/kms-api.service"
SCRIPTS_DIR="/root/AirAccount/kms/scripts"
CLOUDFLARE_DIRS=("/etc/cloudflared" "/root/.cloudflared")

# Rotation policy
KEEP_DAILY=30
KEEP_MONTHLY=12
KEEP_YEARLY=5

# ─── Argument parsing ─────────────────────────────────────────────────────────
FORCE_FULL=false
DEST_OVERRIDE=""
REMOTE=""
DRY_RUN=false

usage() {
    grep '^#' "$0" | grep -v '^#!/' | sed 's/^# \{0,2\}//' | head -40
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --full)       FORCE_FULL=true ;;
        --dest)       DEST_OVERRIDE="$2"; shift ;;
        --dest=*)     DEST_OVERRIDE="${1#*=}" ;;
        --remote)     REMOTE="$2"; shift ;;
        --remote=*)   REMOTE="${1#*=}" ;;
        --dry-run)    DRY_RUN=true ;;
        -h|--help)    usage ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
    shift
done

[[ -n "$DEST_OVERRIDE" ]] && BACKUP_ROOT="$DEST_OVERRIDE"

# ─── Logging setup ────────────────────────────────────────────────────────────
# Tee all stdout/stderr to log file (append)
if [[ "$DRY_RUN" == false ]]; then
    mkdir -p "$(dirname "$LOG_FILE")" 2>/dev/null || true
    exec > >(tee -a "$LOG_FILE") 2>&1
fi

TIMESTAMP=$(date +"%Y-%m-%d_%H%M%S")
START_EPOCH=$(date +%s)

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }
log_section() { echo; echo "=== $* ==="; }

log_section "KMS Backup started — ${TIMESTAMP}"
log "BACKUP_ROOT : ${BACKUP_ROOT}"
log "FORCE_FULL  : ${FORCE_FULL}"
log "REMOTE      : ${REMOTE:-<none>}"
log "DRY_RUN     : ${DRY_RUN}"

# ─── Dry-run guard ────────────────────────────────────────────────────────────
run() {
    if [[ "$DRY_RUN" == true ]]; then
        log "[DRY-RUN] $*"
    else
        "$@"
    fi
}

# ─── Determine backup type and link-dest ─────────────────────────────────────
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
FILES_DIR="${BACKUP_DIR}/files"

# Find the most recent previous backup for incremental link-dest
PREV_BACKUP=""
if [[ "$FORCE_FULL" == false ]]; then
    PREV_BACKUP=$(
        find "$BACKUP_ROOT" -maxdepth 1 -mindepth 1 -type d \
            -name '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]_[0-9]*' \
            2>/dev/null | sort | tail -n1
    ) || true
fi

if [[ -n "$PREV_BACKUP" ]]; then
    BACKUP_TYPE="incremental"
    log "Previous backup found: ${PREV_BACKUP}"
    log "Backup type: incremental (hard-link unchanged files)"
else
    BACKUP_TYPE="full"
    log "No previous backup found (or --full specified)"
    log "Backup type: full"
fi

# ─── Create destination directories ──────────────────────────────────────────
# H-5: backups contain Cloudflare tunnel credentials (cert.pem + credentials.json)
# and wallet metadata. Default umask (022) would make these dirs world-readable
# (0755) so any local user could read the creds. Force private permissions
# (0700) on the backup tree so only the owner (root) can traverse it.
if [[ "$DRY_RUN" == false ]]; then
    mkdir -p "$FILES_DIR"
    chmod 700 "$BACKUP_ROOT" 2>/dev/null || true
    chmod 700 "$BACKUP_DIR" "$FILES_DIR"
fi

# ─── Helper: rsync a source path (file or directory) into FILES_DIR ──────────
# Usage: backup_path <source> <dest_relative_path_under_files/>
backup_path() {
    local src="$1"
    local rel_dest="$2"
    local full_dest="${FILES_DIR}/${rel_dest}"

    if [[ ! -e "$src" ]]; then
        log "SKIP (not found): ${src}"
        return
    fi

    log "Backing up: ${src} -> ${rel_dest}"

    if [[ "$DRY_RUN" == true ]]; then
        log "[DRY-RUN] rsync ${src} -> ${full_dest}"
        return
    fi

    mkdir -p "$(dirname "$full_dest")"

    local rsync_opts=(
        -a                    # archive: recursive, preserve perms/timestamps/links
        --checksum            # compare by checksum, not just mtime (more reliable)
        --delete              # remove files that vanished from source
        --exclude='*.key'
        --exclude='*.pem'
        --exclude='*.priv'
    )

    # Add --link-dest for incremental (saves disk space via hard links).
    # rsync resolves each transferred file's relative path against --link-dest,
    # so --link-dest must point at a DIRECTORY:
    #   - directory source: the previous backup's matching directory
    #   - single-file source: the DIRNAME of the previous file (NOT the file
    #     itself; pointing it at the file makes rsync look for "<file>/<file>"
    #     and the hard-link optimization silently never matches). (H-4)
    if [[ -n "$PREV_BACKUP" && -e "${PREV_BACKUP}/files/${rel_dest}" ]]; then
        if [[ -d "$src" ]]; then
            rsync_opts+=(--link-dest="${PREV_BACKUP}/files/${rel_dest}")
        else
            rsync_opts+=(--link-dest="$(dirname "${PREV_BACKUP}/files/${rel_dest}")")
        fi
    fi

    if [[ -d "$src" ]]; then
        rsync "${rsync_opts[@]}" "${src}/" "${full_dest}/"
    else
        rsync "${rsync_opts[@]}" "$src" "$full_dest"
    fi
}

# ─── Consistent SQLite backup ────────────────────────────────────────────────
# M-6: kms-api-server may be writing kms.db while we back it up. A plain rsync of
# a live SQLite file can capture a torn (half-written) page and yield a corrupt
# backup. Prefer `sqlite3 .backup` (online backup API — consistent snapshot of a
# live DB). If the sqlite3 CLI is unavailable, fall back to copying the DB plus
# its WAL/SHM sidecars together so the WAL can be replayed at restore time.
backup_sqlite_db() {
    local src="$1"
    local rel_dest="$2"
    local full_dest="${FILES_DIR}/${rel_dest}"

    if [[ ! -e "$src" ]]; then
        log "SKIP (not found): ${src}"
        return
    fi

    log "Backing up SQLite DB: ${src} -> ${rel_dest}"

    if [[ "$DRY_RUN" == true ]]; then
        log "[DRY-RUN] consistent sqlite backup ${src} -> ${full_dest}"
        return
    fi

    mkdir -p "$(dirname "$full_dest")"

    if command -v sqlite3 &>/dev/null; then
        # Online backup: consistent even while the DB is being written.
        if sqlite3 "$src" ".backup '${full_dest}'" 2>/dev/null; then
            chmod 600 "$full_dest" 2>/dev/null || true
            log "  sqlite3 .backup OK"
            return
        fi
        log "  WARNING: sqlite3 .backup failed — falling back to file copy + WAL/SHM"
    else
        log "  sqlite3 CLI not available — copying DB + WAL/SHM sidecars"
    fi

    # Fallback: copy the main DB and any WAL/SHM so a restore can replay the WAL.
    cp -p "$src" "$full_dest"
    for ext in -wal -shm; do
        if [[ -e "${src}${ext}" ]]; then
            cp -p "${src}${ext}" "${full_dest}${ext}"
        fi
    done
    chmod 600 "$full_dest" "${full_dest}-wal" "${full_dest}-shm" 2>/dev/null || true
}

# ─── Gather system info ───────────────────────────────────────────────────────
collect_system_info() {
    local info_file="${FILES_DIR}/system-info.txt"
    log "Collecting system info -> system-info.txt"
    if [[ "$DRY_RUN" == true ]]; then return; fi

    {
        echo "=== KMS System Snapshot ==="
        echo "Timestamp      : ${TIMESTAMP}"
        echo "Hostname       : $(hostname 2>/dev/null || echo unknown)"
        echo "Kernel         : $(uname -r 2>/dev/null || echo unknown)"
        echo "Arch           : $(uname -m 2>/dev/null || echo unknown)"
        echo ""
        echo "=== OP-TEE version ==="
        if [[ -f /proc/version_signature ]]; then
            cat /proc/version_signature
        fi
        if command -v tee-supplicant &>/dev/null; then
            tee-supplicant --version 2>/dev/null || echo "tee-supplicant: version unknown"
        fi
        for f in /sys/bus/platform/devices/*.optee/optee_version \
                  /sys/firmware/optee/uuid; do
            [[ -f "$f" ]] && echo "$f: $(cat "$f")"
        done

        echo ""
        echo "=== kms-api.service status ==="
        systemctl status kms-api.service --no-pager 2>/dev/null \
            || echo "(systemctl not available or service not found)"

        echo ""
        echo "=== kms-api-server binary info ==="
        if [[ -f "$CA_PATH" ]]; then
            ls -lh "$CA_PATH"
            file "$CA_PATH" 2>/dev/null || true
            sha256sum "$CA_PATH" 2>/dev/null || true
        else
            echo "CA binary not found at: ${CA_PATH}"
        fi

        echo ""
        echo "=== TA binary info ==="
        if [[ -f "$TA_PATH" ]]; then
            ls -lh "$TA_PATH"
            sha256sum "$TA_PATH" 2>/dev/null || true
        else
            echo "TA binary not found at: ${TA_PATH}"
        fi

        echo ""
        echo "=== kms.db info ==="
        if [[ -f "$DB_PATH" ]]; then
            ls -lh "$DB_PATH"
            if command -v sqlite3 &>/dev/null; then
                echo "Wallet count: $(sqlite3 "$DB_PATH" 'SELECT COUNT(*) FROM wallets;' 2>/dev/null || echo unknown)"
                echo "Tables: $(sqlite3 "$DB_PATH" '.tables' 2>/dev/null || echo unknown)"
            fi
        else
            echo "kms.db not found at: ${DB_PATH}"
        fi

        echo ""
        echo "=== Disk usage ==="
        df -h /root 2>/dev/null || true

    } > "$info_file"
}

# ─── Main backup logic ────────────────────────────────────────────────────────
log_section "Backing up files"

# 1. SQLite wallet metadata DB (consistent online backup — see M-6)
backup_sqlite_db "$DB_PATH"      "root/AirAccount/kms.db"

# 2. TA binary
backup_path "$TA_PATH"           "lib/optee_armtz/${TA_UUID}.ta"

# 3. CA binary
backup_path "$CA_PATH"           "root/AirAccount/target/release/kms-api-server"

# 4. systemd service config
backup_path "$SERVICE_PATH"      "etc/systemd/system/kms-api.service"

# 5. Cloudflared tunnel config (first location found)
for cf_dir in "${CLOUDFLARE_DIRS[@]}"; do
    if [[ -d "$cf_dir" ]]; then
        rel="${cf_dir#/}"
        backup_path "$cf_dir" "$rel"
        break
    fi
done

# 6. Deployment scripts
backup_path "$SCRIPTS_DIR"       "root/AirAccount/kms/scripts"

# 7. System info snapshot
collect_system_info

# ─── Harden permissions on backed-up sensitive material ───────────────────────
# H-5: ensure no file in the backup tree is group/other-readable, and lock the
# Cloudflare credentials and DB down to owner-only. Belt-and-suspenders on top
# of the 0700 directory perms above.
if [[ "$DRY_RUN" == false ]]; then
    find "$FILES_DIR" -type d -exec chmod 700 {} + 2>/dev/null || true
    find "$FILES_DIR" -type f -exec chmod 600 {} + 2>/dev/null || true
fi

# ─── Generate SHA-256 manifest ────────────────────────────────────────────────
log_section "Generating SHA-256 manifest"
MANIFEST="${BACKUP_DIR}/manifest.sha256"

if [[ "$DRY_RUN" == false ]]; then
    (
        cd "$BACKUP_DIR"
        find files/ -type f | sort | xargs sha256sum
    ) > "$MANIFEST"
    FILE_COUNT=$(wc -l < "$MANIFEST")
    log "Manifest written: ${FILE_COUNT} files -> ${MANIFEST}"
else
    log "[DRY-RUN] Would write manifest at ${MANIFEST}"
    FILE_COUNT=0
fi

# ─── Write backup-info.txt ────────────────────────────────────────────────────
END_EPOCH=$(date +%s)
DURATION=$(( END_EPOCH - START_EPOCH ))

if [[ "$DRY_RUN" == false ]]; then
    BACKUP_SIZE=$(du -sh "${BACKUP_DIR}" 2>/dev/null | cut -f1 || echo "unknown")
    {
        echo "backup_type   = ${BACKUP_TYPE}"
        echo "timestamp     = ${TIMESTAMP}"
        echo "duration_sec  = ${DURATION}"
        echo "file_count    = ${FILE_COUNT}"
        echo "backup_size   = ${BACKUP_SIZE}"
        echo "prev_backup   = ${PREV_BACKUP:-<none>}"
        echo "remote        = ${REMOTE:-<none>}"
        echo "force_full    = ${FORCE_FULL}"
    } > "${BACKUP_DIR}/backup-info.txt"
fi

# ─── Update 'latest' symlink ──────────────────────────────────────────────────
if [[ "$DRY_RUN" == false ]]; then
    ln -sfn "$TIMESTAMP" "${BACKUP_ROOT}/latest"
    log "Updated symlink: ${BACKUP_ROOT}/latest -> ${TIMESTAMP}"
fi

# ─── Remote push ──────────────────────────────────────────────────────────────
if [[ -n "$REMOTE" ]]; then
    log_section "Pushing to remote: ${REMOTE}"
    if [[ "$DRY_RUN" == true ]]; then
        log "[DRY-RUN] rsync -az --delete ${BACKUP_DIR}/ ${REMOTE}/${TIMESTAMP}/"
    else
        rsync -az --delete "${BACKUP_DIR}/" "${REMOTE}/${TIMESTAMP}/" \
            && log "Remote push complete: ${REMOTE}/${TIMESTAMP}/" \
            || log "WARNING: Remote push failed (local backup is intact)"
    fi
fi

# ─── Rotation ─────────────────────────────────────────────────────────────────
rotate_backups() {
    log_section "Rotating old backups"
    local all_backups
    mapfile -t all_backups < <(
        find "$BACKUP_ROOT" -maxdepth 1 -mindepth 1 -type d \
            -name '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]_[0-9]*' \
            2>/dev/null | sort
    )

    local total=${#all_backups[@]}
    log "Total backups before rotation: ${total}"

    declare -A keep_set

    # Keep last N daily (newest N entries)
    local daily_kept=0
    for (( i=${#all_backups[@]}-1; i>=0; i-- )); do
        [[ $daily_kept -ge $KEEP_DAILY ]] && break
        keep_set["${all_backups[$i]}"]=1
        (( daily_kept++ )) || true
    done

    # Keep last N monthly (one per calendar month, newest)
    declare -A monthly_seen
    local monthly_kept=0
    for (( i=${#all_backups[@]}-1; i>=0; i-- )); do
        [[ $monthly_kept -ge $KEEP_MONTHLY ]] && break
        local bname
        bname=$(basename "${all_backups[$i]}")
        local month="${bname:0:7}"
        if [[ -z "${monthly_seen[$month]+_}" ]]; then
            monthly_seen["$month"]=1
            keep_set["${all_backups[$i]}"]=1
            (( monthly_kept++ )) || true
        fi
    done

    # Keep last N yearly (one per calendar year, newest)
    declare -A yearly_seen
    local yearly_kept=0
    for (( i=${#all_backups[@]}-1; i>=0; i-- )); do
        [[ $yearly_kept -ge $KEEP_YEARLY ]] && break
        local bname
        bname=$(basename "${all_backups[$i]}")
        local year="${bname:0:4}"
        if [[ -z "${yearly_seen[$year]+_}" ]]; then
            yearly_seen["$year"]=1
            keep_set["${all_backups[$i]}"]=1
            (( yearly_kept++ )) || true
        fi
    done

    # Delete anything not in keep_set
    local deleted=0
    for bdir in "${all_backups[@]}"; do
        if [[ -z "${keep_set[$bdir]+_}" ]]; then
            log "Deleting old backup: $(basename "$bdir")"
            if [[ "$DRY_RUN" == false ]]; then
                rm -rf "$bdir"
                (( deleted++ )) || true
            else
                log "[DRY-RUN] Would delete: $bdir"
            fi
        fi
    done
    log "Rotation complete: deleted ${deleted}, kept $(( ${#all_backups[@]} - deleted ))"
}

if [[ "$DRY_RUN" == false ]]; then
    rotate_backups
fi

# ─── Summary ──────────────────────────────────────────────────────────────────
log_section "Backup summary"
log "Type     : ${BACKUP_TYPE}"
log "Location : ${BACKUP_DIR}"
log "Files    : ${FILE_COUNT}"
log "Duration : ${DURATION}s"
if [[ "$DRY_RUN" == false ]]; then
    log "Size     : ${BACKUP_SIZE}"
fi
[[ -n "$REMOTE" ]] && log "Remote   : ${REMOTE}"
log "Done."
