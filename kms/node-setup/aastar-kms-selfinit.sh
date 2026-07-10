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
# KMS_KEEPER_ENABLE=1) present in the running kms-api environment. This script turns
# it back off when done.
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

# ── kms.env idempotent upsert helpers (atomic, 0600) ──
env_get() { [ -f "$KMS_ENV" ] && grep -E "^$1=" "$KMS_ENV" | head -1 | cut -d= -f2- || true; }
env_set() {
  local k="$1" v="$2" tmp
  tmp="$(mktemp)"
  { [ -f "$KMS_ENV" ] && grep -v -E "^$k=" "$KMS_ENV" || true; } > "$tmp"
  printf '%s=%s\n' "$k" "$v" >> "$tmp"
  install -m 600 "$tmp" "$KMS_ENV"; rm -f "$tmp"
}
env_del() {
  [ -f "$KMS_ENV" ] || return 0
  local k="$1" tmp; tmp="$(mktemp)"
  grep -v -E "^$k=" "$KMS_ENV" > "$tmp" || true
  install -m 600 "$tmp" "$KMS_ENV"; rm -f "$tmp"
}
# extract a JSON string field without a python dep on the field name being present
json_str() { python3 -c "import sys,json;print(json.load(sys.stdin).get('$1',''))" 2>/dev/null; }

# ── 1. shared signer token (generate once, persisted) ──
TOKEN="$(env_get KMS_BLS_SIGNER_TOKEN)"
if [ -z "$TOKEN" ]; then
  TOKEN="$(openssl rand -hex 32)"
  env_set KMS_BLS_SIGNER_TOKEN "$TOKEN"
  log "generated BLS signer token"
fi

# ── wait for the loopback signer to come up ──
up=0
for _ in $(seq 1 30); do
  if curl -sf -m2 "$SIGNER/health" >/dev/null 2>&1; then up=1; break; fi
  sleep 1
done
[ "$up" = 1 ] || die "signer $SIGNER not up after 30s — is kms-api running?"

# ── 2. BLS provisioning (idempotent) ──
BLS_KEY_ID="$(env_get KMS_BLS_KEY_ID)"
if [ -n "$BLS_KEY_ID" ]; then
  log "BLS key already provisioned ($BLS_KEY_ID); skip"
else
  log "provisioning TEE BLS key via /gen-key ..."
  resp="$(curl -s -m20 -X POST "$SIGNER/gen-key" -H "x-signer-token: $TOKEN")"
  if printf '%s' "$resp" | grep -q '"key_id"'; then
    BLS_KEY_ID="$(printf '%s' "$resp" | json_str key_id)"
    bls_pub="$(printf '%s' "$resp" | json_str public_key)"
    [ -n "$BLS_KEY_ID" ] && [ -n "$bls_pub" ] || die "gen-key returned malformed json: $resp"
    env_set KMS_BLS_KEY_ID "$BLS_KEY_ID"
    env_set KMS_BLS_PUBKEY "$bls_pub"
    log "BLS provisioned: key_id=$BLS_KEY_ID"
  elif printf '%s' "$resp" | grep -q "already exists"; then
    # A sealed TEE BLS singleton exists but its key_id is not recorded in kms.env.
    # We refuse to guess — the operator must recover the key_id or remove the stale
    # singleton (there is no silent-overwrite path; the TA enforces a singleton).
    die "a TEE BLS singleton already exists but KMS_BLS_KEY_ID is unrecorded — \
manual resolution needed (recover the key_id, or remove the stale singleton). resp: $resp"
  else
    die "gen-key failed: $resp"
  fi
fi
BLS_PUBKEY="$(env_get KMS_BLS_PUBKEY)"

# ── 3. keeper EOA provisioning (opt-in, CC-34; graceful if the binary predates it) ──
KEEPER_ADDR=""
if [ "${KMS_KEEPER_ENABLE:-0}" = "1" ]; then
  KTOKEN="$(env_get KMS_KEEPER_SIGNER_TOKEN)"
  if [ -z "$KTOKEN" ]; then
    KTOKEN="$(openssl rand -hex 32)"
    env_set KMS_KEEPER_SIGNER_TOKEN "$KTOKEN"
    log "generated keeper signer token"
  fi
  KEEPER_ADDR="$(env_get KMS_KEEPER_ADDRESS)"
  if [ -n "$(env_get KMS_KEEPER_KEY_ID)" ] && [ -n "$KEEPER_ADDR" ]; then
    log "keeper already provisioned ($KEEPER_ADDR); skip"
  else
    log "provisioning keeper EOA via /kms/gen-keeper-eoa ..."
    kresp="$(curl -s -m20 -X POST "$SIGNER/kms/gen-keeper-eoa" -H "x-signer-token: $KTOKEN" || true)"
    if printf '%s' "$kresp" | grep -q '"address"'; then
      k_id="$(printf '%s' "$kresp" | json_str key_id)"
      KEEPER_ADDR="$(printf '%s' "$kresp" | json_str address)"
      env_set KMS_KEEPER_KEY_ID "$k_id"
      env_set KMS_KEEPER_ADDRESS "$KEEPER_ADDR"
      log "keeper provisioned: $KEEPER_ADDR (fund this EOA)"
    else
      # Binary may predate CC-34 (no /kms/gen-keeper-eoa route → 404). Keeper is
      # optional for the BLS unattended run, so warn and continue rather than fail.
      log "WARN: keeper provisioning skipped (endpoint unavailable/failed): $kresp"
      KEEPER_ADDR=""
    fi
  fi
fi

# ── 4. emit DVT handoff (KMS produces; @repo:dvt consumes into its node_state/dvt.env) ──
umask 077
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
} > "$HANDOFF"
chmod 640 "$HANDOFF"
log "wrote DVT handoff: $HANDOFF (BLS pubkey + shared signer token${KEEPER_ADDR:+ + keeper})"

# ── 5. close the provisioning gate + restart kms-api to load the recorded key ids ──
env_del KMS_BLS_PROVISIONING
env_del KMS_KEEPER_PROVISIONING
rm -f /etc/systemd/system/kms-api.service.d/prov.conf 2>/dev/null || true
systemctl daemon-reload 2>/dev/null || true
log "closed provisioning gate; restarting kms-api to load key ids"
systemctl restart kms-api.service 2>/dev/null || log "WARN: kms-api restart failed (restart it manually)"

# ── 6. mark done ──
: > "$MARKER"; chmod 600 "$MARKER"
log "self-init complete — KMS provisioned; @repo:dvt consumes $HANDOFF for its node_state/dvt.env"
