# KMS Backup System

This document describes the backup strategy for the AirAccount KMS service running on NXP FRDM-IMX93 (aarch64 + OP-TEE TrustZone).

## What is backed up — and why

| Item | Path on device | Why |
|------|---------------|-----|
| Wallet metadata DB | `/root/AirAccount/kms.db` | SQLite database containing wallet addresses and IDs. Needed to reconstruct which keys exist without re-creating all accounts. Contains **no private keys**. |
| TA binary | `/lib/optee_armtz/4319f351-0b24-4097-b659-80ee4f824cdd.ta` | The Trusted Application binary that runs inside OP-TEE. Hard to rebuild without the full OP-TEE SDK toolchain. |
| CA binary | `/root/AirAccount/target/release/kms-api-server` | The compiled host-side HTTP server. Avoids a full Rust cross-compile on restore. |
| systemd service | `/etc/systemd/system/kms-api.service` | Service unit with ExecStartPre dirf.db guard and all environment config. |
| cloudflared config | `/etc/cloudflared/` or `/root/.cloudflared/` | Cloudflare Tunnel credentials and config; required to re-establish the public HTTPS endpoint. |
| Deployment scripts | `/root/AirAccount/kms/scripts/` | All operational scripts (build, deploy, backup, test). |
| System info snapshot | `system-info.txt` in each backup | Kernel version, OP-TEE version, service status, DB row counts — for forensic comparison after a failure. |

## What is NOT backed up — and why

| Item | Path | Why excluded |
|------|------|-------------|
| TEE secure storage | `/var/lib/tee/` | Contains the encrypted private key blobs. The encryption is hardware-bound to the specific IMX93 board's ELE (Edge Lock Enclave) and RPMB partition. Copying these files to any other device — or even back to the same device after a re-flash — yields **unusable ciphertext**. Backing them up would give a false sense of security while providing no recovery value. |
| RPMB partition | `/dev/mmcblk0rpmb` | Hardware-bound replay-protected storage. Cannot be block-copied meaningfully. |
| Any `*.key`, `*.pem`, `*.priv` files | anywhere | Explicitly excluded by rsync `--exclude` rules as a safety net. Private keys must never leave the TEE. |

**Bottom line on private keys**: The TEE is intentionally designed so private keys cannot be extracted. This means they also cannot be backed up to an external medium. Backup is therefore "best-effort metadata recovery" — the actual key material is tied to the hardware.

## Backup strategy

### Incremental via rsync `--link-dest`

On the first run, a full backup is created. All subsequent runs are incremental: rsync compares checksums and hard-links unchanged files from the previous backup, only copying truly new or modified content. This means:

- Each backup directory appears complete (all files present)
- Disk usage grows only by the delta between runs
- Any backup can be used for restore without depending on earlier ones

### Storage layout

```
/root/backups/kms/
  2026-01-15_030000/         <- timestamped backup
    files/                   <- mirrored file tree (relative to /)
      root/AirAccount/kms.db
      lib/optee_armtz/<uuid>.ta
      root/AirAccount/target/release/kms-api-server
      etc/systemd/system/kms-api.service
      etc/cloudflared/       (if present)
      root/AirAccount/kms/scripts/
      system-info.txt
    manifest.sha256          <- SHA-256 of every file in files/
    backup-info.txt          <- type, duration, file count, size
  2026-01-15_040000/         <- next hourly backup
    ...
  latest -> 2026-01-15_040000  <- symlink to newest
```

### Rotation policy

| Tier | Kept |
|------|------|
| Daily | Last 30 backups (regardless of calendar day) |
| Monthly | One backup per calendar month, newest in that month, last 12 months |
| Yearly | One backup per calendar year, newest in that year, last 5 years |

Rotation runs automatically at the end of each backup. A backup kept by any tier is not deleted.

## How to trigger a manual backup

```bash
# Standard incremental backup
/root/AirAccount/kms/scripts/backup.sh

# Force a full backup (ignore previous backup for link-dest)
/root/AirAccount/kms/scripts/backup.sh --full

# Backup to a custom location
/root/AirAccount/kms/scripts/backup.sh --dest /mnt/usb/kms-backups

# Dry-run: see what would happen without touching the filesystem
/root/AirAccount/kms/scripts/backup.sh --dry-run
```

All output is logged to `/var/log/kms-backup.log`.

## How to list and restore from a backup

```bash
# List all available backups
/root/AirAccount/kms/scripts/backup-restore.sh --list

# Dry-run restore of the latest backup (shows what would change, touches nothing)
/root/AirAccount/kms/scripts/backup-restore.sh --backup latest --dry-run

# Restore latest backup to original paths (with confirmation prompt)
/root/AirAccount/kms/scripts/backup-restore.sh --backup latest

# Restore a specific backup
/root/AirAccount/kms/scripts/backup-restore.sh --backup 2026-01-15_030000

# Restore to a staging directory for inspection (no overwrite of live files)
/root/AirAccount/kms/scripts/backup-restore.sh \
  --backup latest \
  --dest /tmp/kms-restore-test

# Skip SHA-256 verification (not recommended)
/root/AirAccount/kms/scripts/backup-restore.sh --backup latest --no-verify
```

### Post-restore steps

After restoring to `/` (the live system):

1. **Reload systemd** — the restore script does this automatically, but verify:
   ```bash
   systemctl daemon-reload
   ```

2. **Restart tee-supplicant** — OP-TEE may have cached the old TA in memory:
   ```bash
   systemctl restart tee-supplicant
   ```

3. **Restart kms-api**:
   ```bash
   systemctl restart kms-api
   ```

4. **Verify the service is healthy**:
   ```bash
   curl -s http://localhost:8080/health
   ```

### Restoring to a different board

If you are restoring to a **different physical IMX93 board**:

- The CA binary, TA binary, service config, and scripts will restore and work fine.
- **kms.db** will restore, but the wallet entries reference keys in the old board's TEE. Those keys **cannot be recovered** — new keys must be created with `CreateKey`.
- The existing wallet addresses (public keys / Ethereum addresses) from kms.db are still useful as a reference for which accounts existed.

## How to set up remote push backup

Remote push uses rsync over SSH. Set up key-based SSH auth first:

```bash
# On the IMX93 board, generate a key for backup pushes
ssh-keygen -t ed25519 -f /root/.ssh/kms_backup_key -N ""

# Copy the public key to the remote server
ssh-copy-id -i /root/.ssh/kms_backup_key.pub user@backup-server.example.com
```

### Manual remote push

```bash
/root/AirAccount/kms/scripts/backup.sh \
  --remote user@backup-server.example.com:/backups/kms-imx93
```

The backup is always written locally first; remote push is a best-effort secondary copy. If the push fails, a warning is logged and the local backup remains intact.

### Automated remote push via the systemd timer

```bash
# Install the hourly timer with remote push configured
/root/AirAccount/kms/scripts/install-backup-timer.sh \
  --remote user@backup-server.example.com:/backups/kms-imx93
```

If you need to update the remote after installation, re-run `install-backup-timer.sh` with the new `--remote` argument — it will overwrite the service unit and restart the timer.

## How to install/manage the automated hourly backup

```bash
# Install hourly backup timer
/root/AirAccount/kms/scripts/install-backup-timer.sh

# Install with remote push
/root/AirAccount/kms/scripts/install-backup-timer.sh \
  --remote user@host:/path

# Check timer status
/root/AirAccount/kms/scripts/install-backup-timer.sh --status
# or directly:
systemctl status kms-backup.timer
systemctl list-timers kms-backup.timer

# Trigger a backup run immediately (useful for testing)
systemctl start kms-backup.service

# View logs
journalctl -u kms-backup --no-pager -n 50
tail -f /var/log/kms-backup.log

# Uninstall the timer
/root/AirAccount/kms/scripts/install-backup-timer.sh --uninstall
```

The timer fires:
- 5 minutes after each boot (`OnBootSec=5min`)
- Every hour thereafter (`OnUnitActiveSec=1h`)
- If the board was off during a scheduled run, it runs immediately on next boot (`Persistent=true`)

## Security considerations

### What an attacker gains from a stolen backup

A backup contains:
- Wallet **addresses** (public, no secret value)
- Wallet **IDs** (internal UUIDs, no cryptographic value)
- The TA binary (already public in the repo)
- The CA binary (already public in the repo)
- Configuration files (contain Cloudflare tunnel credentials — see below)

**Private keys are not present in any backup.** Even if a backup device is stolen, the attacker cannot sign Ethereum transactions.

### Cloudflare tunnel credentials

The cloudflared config directory may contain a `cert.pem` and/or a tunnel credentials JSON file. These allow the holder to establish the Cloudflare Tunnel — i.e., expose the KMS API endpoint without knowing the device's IP address.

Mitigations:
- Store backups on encrypted storage (LUKS, encrypted NFS, etc.)
- Restrict SSH access to the backup server to the IMX93's dedicated backup key
- Rotate cloudflared credentials if a backup is compromised: `cloudflared tunnel delete` + re-register

### Backup storage permissions

```bash
# Lock down the backup directory
chmod 700 /root/backups
chmod 700 /root/backups/kms
```

The backup log at `/var/log/kms-backup.log` contains file sizes and DB row counts but no key material. It can safely be world-readable if needed for monitoring.

### Integrity verification

Every backup includes a `manifest.sha256` file. The restore script verifies this by default. To manually verify:

```bash
cd /root/backups/kms/2026-01-15_030000
sha256sum --check manifest.sha256
```

All files should report `OK`. Any `FAILED` line indicates corruption or tampering.
