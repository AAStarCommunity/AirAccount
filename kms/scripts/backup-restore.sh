#!/usr/bin/env bash
# =============================================================================
# backup-restore.sh — Restore KMS CA/TA and metadata from a backup
#
# Usage:
#   backup-restore.sh [OPTIONS]
#
# Options:
#   --list                  List available backups and exit
#   --backup TIMESTAMP      Select backup by timestamp (YYYY-MM-DD_HHMMSS)
#                           If omitted, prompts interactively or uses 'latest'
#   --dest /path            Destination root to restore into
#                           (default: / — restores to original paths)
#   --src /path             Backup source root (default: /root/backups/kms)
#   --no-verify             Skip SHA-256 manifest verification
#   --dry-run               Show what would be restored without touching the FS
#   -h, --help              Show this help
#
# Safety:
#   - NEVER restores /var/lib/tee/ — TEE secure storage is hardware-bound
#   - NEVER restores *.key, *.pem, *.priv
#   - Verifies SHA-256 manifest by default before restoring
#   - Dry-run mode always available
#
# Examples:
#   # List available backups
#   backup-restore.sh --list
#
#   # Restore latest backup to original paths (/)
#   backup-restore.sh --backup latest
#
#   # Restore specific backup to /tmp/restore-test (for inspection)
#   backup-restore.sh --backup 2026-01-15_030000 --dest /tmp/restore-test
#
#   # Dry-run: see what would be restored without touching anything
#   backup-restore.sh --backup latest --dry-run
# =============================================================================

set -euo pipefail

BACKUP_ROOT="/root/backups/kms"
DEST_ROOT="/"
SELECTED_BACKUP=""
DRY_RUN=false
VERIFY=true
LIST_ONLY=false
LOG_FILE="/var/log/kms-backup.log"

# ─── Argument parsing ─────────────────────────────────────────────────────────
usage() {
    grep '^#' "$0" | grep -v '^#!/' | sed 's/^# \{0,2\}//' | head -40
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --list)          LIST_ONLY=true ;;
        --backup)        SELECTED_BACKUP="$2"; shift ;;
        --backup=*)      SELECTED_BACKUP="${1#*=}" ;;
        --dest)          DEST_ROOT="$2"; shift ;;
        --dest=*)        DEST_ROOT="${1#*=}" ;;
        --src)           BACKUP_ROOT="$2"; shift ;;
        --src=*)         BACKUP_ROOT="${1#*=}" ;;
        --no-verify)     VERIFY=false ;;
        --dry-run)       DRY_RUN=true ;;
        -h|--help)       usage ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
    shift
done

# ─── Logging ──────────────────────────────────────────────────────────────────
if [[ "$DRY_RUN" == false && "$LIST_ONLY" == false ]]; then
    mkdir -p "$(dirname "$LOG_FILE")" 2>/dev/null || true
    exec > >(tee -a "$LOG_FILE") 2>&1
fi

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }
log_section() { echo; echo "=== $* ==="; }

# ─── List backups ─────────────────────────────────────────────────────────────
list_backups() {
    echo ""
    echo "Available KMS backups in: ${BACKUP_ROOT}"
    echo "────────────────────────────────────────────────────"
    local found=0
    while IFS= read -r -d '' bdir; do
        local bname
        bname=$(basename "$bdir")
        local info_file="${bdir}/backup-info.txt"
        local type="unknown" size="?" files="?"
        if [[ -f "$info_file" ]]; then
            type=$(grep '^backup_type' "$info_file" 2>/dev/null | cut -d= -f2 | tr -d ' ' || echo unknown)
            size=$(grep '^backup_size' "$info_file" 2>/dev/null | cut -d= -f2 | tr -d ' ' || echo ?)
            files=$(grep '^file_count' "$info_file" 2>/dev/null | cut -d= -f2 | tr -d ' ' || echo ?)
        fi
        local is_latest=""
        local latest_link="${BACKUP_ROOT}/latest"
        if [[ -L "$latest_link" ]]; then
            local target
            target=$(readlink "$latest_link" 2>/dev/null || echo "")
            [[ "$target" == "$bname" ]] && is_latest=" [LATEST]"
        fi
        printf "  %-28s  type=%-12s  files=%-5s  size=%s%s\n" \
            "$bname" "$type" "$files" "$size" "$is_latest"
        (( found++ )) || true
    done < <(
        find "$BACKUP_ROOT" -maxdepth 1 -mindepth 1 -type d \
            -name '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]_[0-9]*' \
            2>/dev/null | sort | tr '\n' '\0'
    )
    echo "────────────────────────────────────────────────────"
    echo "Total: ${found} backup(s)"
    echo ""
    echo "To restore the latest backup:"
    echo "  $0 --backup latest [--dest /] [--dry-run]"
    echo ""
}

if [[ "$LIST_ONLY" == true ]]; then
    list_backups
    exit 0
fi

# ─── Resolve backup directory ─────────────────────────────────────────────────
if [[ -z "$SELECTED_BACKUP" ]]; then
    # Default to 'latest' symlink if it exists
    if [[ -L "${BACKUP_ROOT}/latest" ]]; then
        SELECTED_BACKUP="latest"
        log "No --backup specified, defaulting to 'latest'"
    else
        echo "ERROR: No backup specified and no 'latest' symlink found." >&2
        echo "Use --list to see available backups, then --backup TIMESTAMP" >&2
        exit 1
    fi
fi

if [[ "$SELECTED_BACKUP" == "latest" ]]; then
    if [[ ! -L "${BACKUP_ROOT}/latest" ]]; then
        echo "ERROR: No 'latest' symlink found in ${BACKUP_ROOT}" >&2
        exit 1
    fi
    BACKUP_TS=$(readlink "${BACKUP_ROOT}/latest")
    BACKUP_DIR="${BACKUP_ROOT}/${BACKUP_TS}"
else
    BACKUP_TS="$SELECTED_BACKUP"
    BACKUP_DIR="${BACKUP_ROOT}/${BACKUP_TS}"
fi

if [[ ! -d "$BACKUP_DIR" ]]; then
    echo "ERROR: Backup directory not found: ${BACKUP_DIR}" >&2
    echo "Use --list to see available backups." >&2
    exit 1
fi

FILES_DIR="${BACKUP_DIR}/files"
MANIFEST="${BACKUP_DIR}/manifest.sha256"

log_section "KMS Restore — $(date '+%Y-%m-%d %H:%M:%S')"
log "Backup   : ${BACKUP_TS}"
log "From     : ${BACKUP_DIR}"
log "Dest     : ${DEST_ROOT}"
log "DRY_RUN  : ${DRY_RUN}"
log "VERIFY   : ${VERIFY}"

# Show backup info
if [[ -f "${BACKUP_DIR}/backup-info.txt" ]]; then
    echo ""
    echo "--- Backup info ---"
    cat "${BACKUP_DIR}/backup-info.txt"
    echo "-------------------"
fi

# ─── SHA-256 Manifest verification ───────────────────────────────────────────
if [[ "$VERIFY" == true ]]; then
    log_section "Verifying SHA-256 manifest"
    if [[ ! -f "$MANIFEST" ]]; then
        echo "ERROR: Manifest file not found: ${MANIFEST}" >&2
        echo "Use --no-verify to skip verification (not recommended)." >&2
        exit 1
    fi
    (
        cd "$BACKUP_DIR"
        if sha256sum --check "$MANIFEST" --quiet 2>/dev/null; then
            log "Manifest verification PASSED ($(wc -l < "$MANIFEST") files OK)"
        else
            echo "ERROR: Manifest verification FAILED — backup may be corrupt!" >&2
            echo "Run: cd ${BACKUP_DIR} && sha256sum --check manifest.sha256" >&2
            exit 1
        fi
    )
else
    log "WARNING: Skipping manifest verification (--no-verify)"
fi

# ─── Safety: never restore TEE secure storage or key files ───────────────────
NEVER_RESTORE_PATTERNS=(
    "*/var/lib/tee/*"
    "*.key"
    "*.pem"
    "*.priv"
)

# M-7: build the exclude args as a real bash array. The previous version did
# `echo "${args[@]}"` and the caller captured it via unquoted `$(...)`, which
# subjects the result to word-splitting AND pathname expansion. If a `*.key` /
# `*.pem` / `*.priv` file happened to exist in the CWD, the shell would expand
# `--exclude=*.key` into the matching filename, silently dropping the exclusion
# and risking restore of private-key material. Populating a global array and
# expanding it quoted (`"${EXCLUDE_ARGS[@]}"`) avoids both hazards.
EXCLUDE_ARGS=()
build_exclude_args() {
    EXCLUDE_ARGS=()
    local pat
    for pat in "${NEVER_RESTORE_PATTERNS[@]}"; do
        EXCLUDE_ARGS+=(--exclude="$pat")
    done
}

# ─── Restore ──────────────────────────────────────────────────────────────────
log_section "Restoring files"

# Safety guard: warn if restoring to /
if [[ "$DEST_ROOT" == "/" && "$DRY_RUN" == false ]]; then
    echo ""
    echo "WARNING: You are about to restore files to the live root filesystem (/)."
    echo "This will OVERWRITE existing files at their original paths."
    echo "Affected paths:"
    find "$FILES_DIR" -maxdepth 4 -type f | sed "s|${FILES_DIR}||" | head -20
    echo ""
    read -r -p "Continue? [yes/N] " confirm
    if [[ "$confirm" != "yes" ]]; then
        echo "Aborted."
        exit 0
    fi
fi

build_exclude_args
RSYNC_OPTS=(
    -a
    --checksum
    "${EXCLUDE_ARGS[@]}"
)

if [[ "$DRY_RUN" == true ]]; then
    RSYNC_OPTS+=(--dry-run --verbose)
fi

restore_path() {
    local rel_src="$1"     # relative path under files/
    local src_full="${FILES_DIR}/${rel_src}"
    local dest_full="${DEST_ROOT%/}/${rel_src}"

    if [[ ! -e "$src_full" ]]; then
        log "SKIP (not in backup): ${rel_src}"
        return
    fi

    log "Restoring: ${rel_src} -> ${dest_full}"

    if [[ "$DRY_RUN" == false ]]; then
        mkdir -p "$(dirname "$dest_full")"
    fi

    if [[ -d "$src_full" ]]; then
        rsync "${RSYNC_OPTS[@]}" "${src_full}/" "${dest_full}/"
    else
        rsync "${RSYNC_OPTS[@]}" "$src_full" "$dest_full"
    fi
}

# Restore each backed-up item
TA_UUID="4319f351-0b24-4097-b659-80ee4f824cdd"
restore_path "root/AirAccount/kms.db"
# M-6: if the backup used the WAL/SHM fallback (no sqlite3 CLI at backup time),
# restore the sidecars too so the WAL can be replayed on first open.
restore_path "root/AirAccount/kms.db-wal"
restore_path "root/AirAccount/kms.db-shm"
restore_path "lib/optee_armtz/${TA_UUID}.ta"
restore_path "root/AirAccount/target/release/kms-api-server"
restore_path "etc/systemd/system/kms-api.service"
restore_path "etc/cloudflared"
restore_path "root/.cloudflared"
restore_path "root/AirAccount/kms/scripts"

# ─── Post-restore steps (skip in dry-run or non-root restore) ─────────────────
if [[ "$DRY_RUN" == false && "$DEST_ROOT" == "/" ]]; then
    log_section "Post-restore steps"

    # Ensure CA binary is executable
    CA_DEST="/root/AirAccount/target/release/kms-api-server"
    if [[ -f "$CA_DEST" ]]; then
        chmod +x "$CA_DEST"
        log "Set executable bit on ${CA_DEST}"
    fi

    # Reload systemd so the restored service file takes effect
    if command -v systemctl &>/dev/null; then
        systemctl daemon-reload 2>/dev/null && log "systemctl daemon-reload done" \
            || log "WARNING: daemon-reload failed (check manually)"
    fi

    log ""
    log "IMPORTANT: The TA binary has been restored, but OP-TEE may have cached the"
    log "old version in memory. You should restart the device or at minimum restart"
    log "tee-supplicant to reload the TA from disk:"
    log "  systemctl restart tee-supplicant"
    log "  systemctl restart kms-api"
    log ""
    log "NOTE: TEE secure storage (/var/lib/tee/) was NOT restored — it is"
    log "hardware-bound and cannot be transferred. If you are restoring to the"
    log "same hardware, the existing secure storage remains in place."
fi

# ─── Summary ──────────────────────────────────────────────────────────────────
log_section "Restore summary"
log "Backup   : ${BACKUP_TS}"
log "Dest     : ${DEST_ROOT}"
log "Verify   : ${VERIFY}"
log "Dry-run  : ${DRY_RUN}"
if [[ "$DRY_RUN" == true ]]; then
    log "(Dry-run mode: no files were actually changed)"
fi
log "Done."
