#!/usr/bin/env bash
# aastar-kms-selfinit.sh — hands-off, idempotent first-boot KMS provisioning.
#
# The "power-on self-run" KMS side: on first boot (with the provisioning gate on),
# this provisions the board's TEE BLS key (+ optional keeper EOA, CC-34), records
# the key ids into /etc/airaccount/kms.env, emits a DVT handoff, then CLOSES the
# provisioning gate and restarts kms-api. Re-running is safe (idempotent) — every
# step is skip-if-already-done and a marker file guards the whole run.
#
# Repo boundary (per jason): KMS-side ONLY. It provisions KMS-owned TEE keys and
# writes /etc/airaccount/{kms.env,dvt-handoff.env}. It does NOT build/deploy DVT,
# nor write DVT's node_state.json / install dir — @repo:dvt consumes the handoff.
#
# First-boot prerequisite (set by the installer): kms-api must be started with the
# provisioning gate on, i.e. KMS_BLS_PROVISIONING=1 (+ KMS_KEEPER_PROVISIONING=1 if
# KMS_KEEPER_ENABLE=1) present in kms.env. This script turns it back off when done.
set -euo pipefail

CONFIG_DIR=/etc/airaccount
KMS_ENV="$CONFIG_DIR/kms.env"
HANDOFF="$CONFIG_DIR/dvt-handoff.env"
MARKER="$CONFIG_DIR/.kms-selfinit-done"
SIGNER="http://127.0.0.1:3100"

log() { echo "[kms-selfinit] $*"; }
die() { echo "[kms-selfinit] FATAL: $*" >&2; exit 2; }

[ "$(id -u)" -eq 0 ] || die "must run as root (writes $CONFIG_DIR, restarts kms-api)"

# ── idempotency: whole-run guard ──
if [ -f "$MARKER" ]; then
  log "already initialized ($MARKER exists); nothing to do"
  exit 0
fi
mkdir -p "$CONFIG_DIR"

# ── value validators (anchored — reject anything with an embedded newline, which
# is the config-injection vector: a signer response field like
# public_key="0x..\nKMS_BLS_PROVISIONING=1" would otherwise write a second line
# into kms.env, re-opening the gate + crossing into the DVT handoff) ──
_is_uuid() { [[ "$1" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$ ]]; }
_is_hex()  { [[ "$1" =~ ^0x[0-9a-fA-F]+$ ]]; } # 0x-prefixed hex; caller checks length

# ── kms.env idempotent upsert helpers (atomic same-dir rename, 0600) ──
_valid_key() { [[ "$1" =~ ^[A-Z_][A-Z0-9_]*$ ]] || die "invalid env key: $1"; }
env_get() {
  _valid_key "$1"
  [ -f "$KMS_ENV" ] && grep -E "^$1=" "$KMS_ENV" | head -1 | cut -d= -f2- || true
}
_env_write() { # replace kms.env atomically from stdin
  local tmp; tmp="$(mktemp "$CONFIG_DIR/.kmsenv.XXXXXX")"
  cat > "$tmp"; chmod 600 "$tmp"; mv -f "$tmp" "$KMS_ENV"
}
env_set() {
  _valid_key "$1"
  # Generic injection guard: a value must be a single line. Blocks any newline/CR
  # from reaching kms.env regardless of source (belt-and-suspenders vs the typed
  # validators applied to each provisioned value below).
  case "$2" in *$'\n'* | *$'\r'*) die "refusing multi-line value for $1 (injection guard)" ;; esac
  local k="$1" v="$2"
  { [ -f "$KMS_ENV" ] && grep -v -E "^$k=" "$KMS_ENV" || true; printf '%s=%s\n' "$k" "$v"; } | _env_write
}
env_del() {
  _valid_key "$1"
  [ -f "$KMS_ENV" ] || return 0
  { grep -v -E "^$1=" "$KMS_ENV" || true; } | _env_write
}
# Extract a JSON string field; degrade to empty (exit 0) on ANY parse error so
# callers under `set -euo pipefail` don't abort on a 404/non-JSON response — the
# empty-field checks then drive the warn/continue path. Field passed via argv.
json_str() {
  python3 -c 'import sys,json
try:
    print(json.load(sys.stdin).get(sys.argv[1], ""))
except Exception:
    print("")' "$1" 2>/dev/null || true
}

wait_signer() {
  for _ in $(seq 1 30); do
    curl -sf -m2 "$SIGNER/health" >/dev/null 2>&1 && return 0
    sleep 1
  done
  die "signer $SIGNER not up after 30s — is kms-api running?"
}
restart_kms() { # $1 = fatal-on-fail (1) or best-effort (0)
  systemctl restart kms-api.service 2>/dev/null && return 0
  if [ "${1:-0}" = 1 ]; then die "kms-api restart failed — refusing to finish (gate may still be live)"; fi
  log "WARN: kms-api restart failed"
}

# ── 1. shared signer tokens (generate once, persisted) ──
if [ -z "$(env_get KMS_BLS_SIGNER_TOKEN)" ]; then
  env_set KMS_BLS_SIGNER_TOKEN "$(openssl rand -hex 32)"
  log "generated BLS signer token"
fi
TOKEN="$(env_get KMS_BLS_SIGNER_TOKEN)"

KEEPER_ENABLE="$(env_get KMS_KEEPER_ENABLE)"
if [ "$KEEPER_ENABLE" = 1 ] && [ -z "$(env_get KMS_KEEPER_SIGNER_TOKEN)" ]; then
  env_set KMS_KEEPER_SIGNER_TOKEN "$(openssl rand -hex 32)"
  log "generated keeper signer token"
fi

# ── 1c. API key (#145 fail-closed): the AWS-KMS-style API rejects every authed
# request with 401 "Missing API key" unless KMS_API_KEY is set (or the node is in
# KMS_ALLOW_OPEN_MODE=1). Auto-provision one so a node is never left in the
# reachable-but-authed-API-locked state — the operator can rotate it later. ──
if [ -z "$(env_get KMS_API_KEY)" ] && [ "$(env_get KMS_ALLOW_OPEN_MODE)" != 1 ]; then
  # Capture + validate the entropy BEFORE writing: a bare `env_set "kms_$(openssl…)"`
  # would, if openssl were missing/failing, silently persist a predictable `kms_` key
  # (command-substitution failure does not abort under set -e inside an argument).
  api_hex="$(openssl rand -hex 24)" || die "failed to generate KMS_API_KEY entropy"
  [[ "$api_hex" =~ ^[0-9a-f]{48}$ ]] || die "invalid KMS_API_KEY entropy from openssl"
  env_set KMS_API_KEY "kms_$api_hex"
  log "generated KMS_API_KEY (fail-closed API was unprovisioned)"
fi

# ── 1d. Drop any legacy systemd drop-in that hard-codes the BLS identity. Early
# boards pinned KMS_BLS_KEY_ID/PUBKEY in /etc/systemd/system/kms-api.service.d/bls.conf;
# that duplicates kms.env (the authoritative source) and, if it ever wins the env
# load order, would activate a stale/rotated key. kms.env is authoritative. ──
_bls_dropin="/etc/systemd/system/kms-api.service.d/bls.conf"
if [ -f "$_bls_dropin" ] && grep -qE "^Environment=KMS_BLS_(KEY_ID|PUBKEY)=" "$_bls_dropin"; then
  rm -f "$_bls_dropin"
  systemctl daemon-reload 2>/dev/null || true
  log "removed legacy kms-api bls.conf drop-in (kms.env is authoritative for KMS_BLS_*)"
fi

# ── 2. restart kms-api so it runs WITH the tokens+gates from kms.env, THEN provision.
# Required because keeper /kms/gen-keeper-eoa is fail-closed: it rejects unless the
# RUNNING process has KMS_KEEPER_SIGNER_TOKEN. (BLS /gen-key would tolerate a token
# mismatch via its localhost default, but we make both correct + explicit.) ──
restart_kms 1
wait_signer

# ── 3. BLS provisioning (idempotent; both key_id AND pubkey must be present) ──
if [ -n "$(env_get KMS_BLS_KEY_ID)" ] && [ -n "$(env_get KMS_BLS_PUBKEY)" ]; then
  log "BLS key already provisioned ($(env_get KMS_BLS_KEY_ID)); skip"
else
  log "provisioning TEE BLS key via /gen-key ..."
  resp="$(curl -s -m20 -X POST "$SIGNER/gen-key" -H "x-signer-token: $TOKEN" || true)"
  if printf '%s' "$resp" | grep -q '"key_id"'; then
    bls_id="$(printf '%s' "$resp" | json_str key_id)"
    bls_pub="$(printf '%s' "$resp" | json_str public_key)"
    [ -n "$bls_id" ] && [ -n "$bls_pub" ] || die "gen-key returned malformed json: $resp"
    # Validate the signer's values before they touch kms.env — a UUID key_id and a
    # 0x+96-hex (48-byte G1) pubkey. Rejects any injection payload (embedded newline
    # fails the anchored regex) and truncation.
    _is_uuid "$bls_id" || die "gen-key returned a non-uuid key_id (rejected): $bls_id"
    { _is_hex "$bls_pub" && [ "${#bls_pub}" -eq 98 ]; } || die "gen-key returned an invalid BLS pubkey (want 0x+96 hex): $bls_pub"
    # pubkey first, then key_id — so "key_id present" always implies "pubkey present".
    env_set KMS_BLS_PUBKEY "$bls_pub"
    env_set KMS_BLS_KEY_ID "$bls_id"
    log "BLS provisioned: key_id=$bls_id"
  elif printf '%s' "$resp" | grep -q "already exists"; then
    # A sealed TEE BLS singleton exists but its key_id is not recorded in kms.env.
    # Refuse to guess — the operator must recover the key_id or remove the stale
    # singleton (the TA enforces a singleton; no silent-overwrite path). Marker is
    # NOT written, so a rerun after manual fix still works.
    die "a TEE BLS singleton already exists but KMS_BLS_KEY_ID is unrecorded — \
manual resolution needed (recover the key_id, or remove the stale singleton). resp: $resp"
  else
    die "gen-key failed: $resp"
  fi
fi
BLS_KEY_ID="$(env_get KMS_BLS_KEY_ID)"
BLS_PUBKEY="$(env_get KMS_BLS_PUBKEY)"

# ── 4. keeper EOA provisioning (opt-in, CC-34; graceful if the binary predates it) ──
KEEPER_ADDR=""
if [ "$KEEPER_ENABLE" = 1 ]; then
  if [ -n "$(env_get KMS_KEEPER_KEY_ID)" ] && [ -n "$(env_get KMS_KEEPER_ADDRESS)" ]; then
    KEEPER_ADDR="$(env_get KMS_KEEPER_ADDRESS)"
    log "keeper already provisioned ($KEEPER_ADDR); skip"
  else
    log "provisioning keeper EOA via /kms/gen-keeper-eoa ..."
    ktok="$(env_get KMS_KEEPER_SIGNER_TOKEN)"
    kresp="$(curl -s -m20 -X POST "$SIGNER/kms/gen-keeper-eoa" -H "x-signer-token: $ktok" || true)"
    k_id="$(printf '%s' "$kresp" | json_str key_id)"
    k_addr="$(printf '%s' "$kresp" | json_str address)"
    if [ -z "$k_id" ] && [ -z "$k_addr" ]; then
      # BOTH absent = endpoint truly unavailable (binary predates CC-34 → 404, or
      # token/gate off). Keeper is optional for the BLS unattended run → warn/continue.
      log "WARN: keeper provisioning skipped (endpoint unavailable/failed): $kresp"
    elif [ -n "$k_id" ] && [ -n "$k_addr" ]; then
      # BOTH present → must be well-formed (UUID key_id + 0x+40-hex 20-byte address).
      _is_uuid "$k_id" || die "keeper gen returned a non-uuid key_id (rejected): $k_id"
      { _is_hex "$k_addr" && [ "${#k_addr}" -eq 42 ]; } || die "keeper gen returned an invalid address (want 0x+40 hex): $k_addr"
      env_set KMS_KEEPER_KEY_ID "$k_id"
      env_set KMS_KEEPER_ADDRESS "$k_addr"
      KEEPER_ADDR="$k_addr"
      log "keeper provisioned: $KEEPER_ADDR (fund this EOA)"
    else
      # PARTIAL (exactly one of key_id/address present) = anomalous/corrupt/attack →
      # fail closed. Not the graceful skip (that is reserved for BOTH absent).
      die "keeper gen returned a partial response (one of key_id/address missing): $kresp"
    fi
  fi
fi

# ── 5. emit DVT handoff (KMS produces; @repo:dvt consumes into its node_state/dvt.env) ──
htmp="$(mktemp "$CONFIG_DIR/.handoff.XXXXXX")"
{
  echo "# AAStar KMS -> DVT handoff (generated by aastar-kms-selfinit; consume, do not hand-edit)"
  echo "RUST_SIGNER_URL=$SIGNER"
  echo "RUST_SIGNER_REQUIRED=true"
  echo "RUST_SIGNER_TOKEN=$TOKEN"
  echo "KMS_BLS_KEY_ID=$BLS_KEY_ID"
  echo "KMS_BLS_PUBKEY=$BLS_PUBKEY"
  if [ -n "$KEEPER_ADDR" ]; then
    echo "KEEPER_SIGNER_URL=$SIGNER/kms/sign"
    echo "KEEPER_SIGNER_TOKEN=$(env_get KMS_KEEPER_SIGNER_TOKEN)"
    echo "KEEPER_ADDRESS=$KEEPER_ADDR"
  fi
} > "$htmp"
chmod 600 "$htmp"; mv -f "$htmp" "$HANDOFF"
log "wrote DVT handoff: $HANDOFF (BLS pubkey + shared signer token${KEEPER_ADDR:+ + keeper})"

# ── 6. close the provisioning gate + restart kms-api to load the recorded key ids.
# Fatal if the restart fails: we must not leave the running process with the gate
# open AND write the marker (which would block a retry). ──
env_del KMS_BLS_PROVISIONING
env_del KMS_KEEPER_PROVISIONING
rm -f /etc/systemd/system/kms-api.service.d/prov.conf 2>/dev/null || true
systemctl daemon-reload 2>/dev/null || true
log "closed provisioning gate; restarting kms-api to load key ids"
restart_kms 1
wait_signer

# ── 7. mark done ──
: > "$MARKER"; chmod 600 "$MARKER"
log "self-init complete — KMS provisioned; @repo:dvt consumes $HANDOFF for its node_state/dvt.env"
