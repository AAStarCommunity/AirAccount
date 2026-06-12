# MX93 Board Deploy Scripts

Scripts for the NXP FRDM-IMX93 (aarch64 Cortex-A55) running AirAccount KMS.

## Normal Deploy Flow

```
Mac (Claude fixes code) → git push → Board runs deploy.sh
```

### Step 1 — One-time board setup

```bash
# On board (via serial /dev/cu.usbmodem5B6D0044901 115200)
bash /root/AirAccount/scripts/mx93/install-git.sh
```

### Step 2 — Deploy after code change

```bash
# Claude calls this via serial after pushing to GitHub
bash /root/AirAccount/scripts/mx93/deploy.sh main
```

This does: `git pull` → `cargo build --release` → `systemctl restart kms-api` → smoke test.

---

## Individual Scripts

| Script | Purpose | Runtime |
|--------|---------|---------|
| `deploy.sh [branch]` | Full pipeline: pull → build → restart → test | ~15 min first run, ~3 min incremental |
| `build-ca.sh [--no-restart]` | Build CA (kms-api-server) release binary only | ~3-15 min |
| `build-ta.sh` | Build TA (requires OP-TEE dev kit, not on board) | N/A until dev kit installed |
| `test-smoke.sh [host:port]` | API smoke test (health, stats, charset, JSON validity) | ~5 sec |
| `purge-tee-entry.sh <uuid>` | Delete TEE + SQLite entry (admin, bypass passkey) | instant |
| `purge-tee-entry.sh --list-test-keys` | List test/gap key candidates | instant |
| `purge-tee-entry.sh --purge-all-test-keys` | Delete all test keys (confirms before running) | instant |
| `install-git.sh` | Install git via opkg | ~30 sec |

---

## TEE Orphan Cleanup

TEE orphans are entries in OP-TEE secure storage that have no matching SQLite row.
They arise from:
1. Gap keys where SQLite row was deleted via ForceRemoveWallet (Issue #41)
2. Failed CreateKey where TEE write succeeded but SQLite write failed

### Known orphan UUIDs (from security testing, 2025)

These gap keys had their SQLite rows deleted but TEE entries remain:
```
# Insert UUIDs here as they're discovered
```

To purge a known orphan:
```bash
bash purge-tee-entry.sh <uuid>
```

### Listing test keys

```bash
bash purge-tee-entry.sh --list-test-keys
```

Shows wallets with test-pattern passkeys (`04000000...`) or test descriptions.

---

## TA vs CA — What Can Be Rebuilt

| Component | Rebuild on board? | Requires |
|-----------|------------------|---------|
| CA (`kms-api-server`) | ✅ Yes, `build-ca.sh` | Rust toolchain (already installed) |
| TA (`*.ta` binary) | ❌ No, needs dev kit | `TA_DEV_KIT_DIR`, `sign_encrypt.py`, `tee_internal_api.h` |

Current TA path: `/usr/lib/optee_armtz/4319f351-0b24-4097-b659-80ee4f824cdd.ta`

---

## How Claude Uses These Scripts

When fixing a bug:
1. Fix code on Mac
2. Commit + push to branch
3. Connect to board via serial: `python3 scripts/mx93/serial-run.py "bash /root/AirAccount/scripts/mx93/deploy.sh feat/my-fix"`
4. Wait for build (~3-15 min) and check output
5. Run smoke test: `python3 scripts/mx93/serial-run.py "bash /root/AirAccount/scripts/mx93/test-smoke.sh"`
