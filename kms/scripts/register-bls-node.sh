#!/usr/bin/env bash
# register-bls-node.sh — BOOTSTRAP-ONLY on-chain registration of the co-located DVT node's
# BLS G1 pubkey (CC-24). It calls validator.registerPublicKey(nodeId, pubkey), which the
# contract ONLY accepts while requireStake == false (owner-registers bootstrap mode).
#
# ⚠️ If the validator has requireStake == true (staked mode, the default for a live
# validator), this path reverts "Staking on: use registerWithProof" — you must instead use
# the STAKED path: DVT's `scripts/register-node.mjs`, which builds a BLS proof-of-possession
# and, for a key-less KMS-TEE node, gets the PoP signed by the KMS via POST :3100/pop
# {node_id, operator} → {pop_signature} (this KMS exposes /pop; the operator must also be
# staked for ROLE_DVT). This script remains handy for a bootstrap/dev validator.
#
# Needs the OPERATOR key (validator owner / authorized registrant), which by design never
# lives on the node. Run it once per node from an operator host.
#
# What it sends (matches DVT blockchain.service.ts registerNodeOnChain, byte-for-byte):
#   validator.registerPublicKey(bytes32 nodeId, bytes publicKey)
# gated by a view pre-check:
#   validator.isRegistered(bytes32 nodeId) -> bool
#
# Requires: foundry `cast`.
#
# ── Node identity (A-board; override via env for another node) ─────────────────
# PUBKEY is the on-chain form the validator requires: EIP-2537 UNCOMPRESSED G1 = 128
# bytes = [16 zero ‖ 48-byte x ‖ 16 zero ‖ 48-byte y]. NOT the 48-byte compressed G1
# from KMS gen-key / node_state.json — expand it first (DVT bls.util encodeG1Point, or
# noble: G1.ProjectivePoint.fromHex(compressed).toAffine() → pad x,y to 48 each).
NODE_ID="${NODE_ID:-0xf177545fd7889a4a0670944da3b3ae2ca12718cec89c830e59a05ebc4b6dd664}"
PUBKEY="${PUBKEY:-0x00000000000000000000000000000000149ffed8a9a5bbd153714a6752f327ff94834c636a1e35acf8af3e405af01a5a0e6f8d6b76987722262b12a9865e1436000000000000000000000000000000000063ccae09f227cd32ec77af7444effcfc46c61b4f977617df5ec8efd7141bf99f025c266ceb695ed63e45a277d23516}"
VALIDATOR="${VALIDATOR:-0x539B9681aFd5BFbCaa655Fe4c6BdcFe1fa7864bC}"
RPC_URL="${RPC_URL:-}"
set -euo pipefail

usage() {
  cat >&2 <<EOF
Usage:
  # TESTNET (Sepolia) — operator key from SuperPaymaster .env.sepolia:
  RPC_URL=<sepolia-rpc> OPERATOR_PK=<0x…> $0
    e.g.  source ~/Dev/aastar/superpaymaster/.env.sepolia
          RPC_URL="\$SEPOLIA_RPC_URL" OPERATOR_PK="\$OWNER_PRIVATE_KEY" $0
    (use whichever key is the validator's authorized operator/owner)

  # MAINNET — cast wallet keystore (key never on disk in plaintext):
  RPC_URL=<mainnet-rpc> CAST_ACCOUNT=<keystore-name> CAST_FROM=<0xoperator> $0

Env overrides: NODE_ID, PUBKEY, VALIDATOR (default = A-board values above).
EOF
  exit 2
}

command -v cast >/dev/null || { echo "‼ foundry 'cast' not found (https://getfoundry.sh)"; exit 1; }
[ -n "$RPC_URL" ] || { echo "‼ RPC_URL required"; usage; }
# length-based checks (portable — avoids bash regex {n}-interval RE_DUP_MAX quirks)
{ [[ "$NODE_ID" =~ ^0x[0-9a-fA-F]+$ ]] && [ ${#NODE_ID} -eq 66 ]; } || { echo "‼ NODE_ID must be 0x+64hex (bytes32)"; exit 1; }
{ [[ "$PUBKEY"  =~ ^0x[0-9a-fA-F]+$ ]] && [ ${#PUBKEY} -eq 258 ]; } || { echo "‼ PUBKEY must be 0x+256hex (128-byte EIP-2537 uncompressed G1) — expand the 48-byte compressed key first"; exit 1; }

echo "validator : $VALIDATOR"
echo "nodeId    : $NODE_ID"
echo "pubkey    : $PUBKEY"

echo "── pre-check: isRegistered(nodeId) ──"
already="$(cast call "$VALIDATOR" 'isRegistered(bytes32)(bool)' "$NODE_ID" --rpc-url "$RPC_URL")"
if [ "$already" = "true" ]; then
  echo "✅ already registered — nothing to do (idempotent)."
  exit 0
fi
echo "not registered yet → sending registerPublicKey ..."

# Build the cast-send auth flags: testnet raw key OR mainnet cast-wallet keystore.
AUTH=()
if [ -n "${OPERATOR_PK:-}" ]; then
  AUTH=(--private-key "$OPERATOR_PK")
elif [ -n "${CAST_ACCOUNT:-}" ]; then
  AUTH=(--account "$CAST_ACCOUNT" ${CAST_FROM:+--from "$CAST_FROM"})
else
  echo "‼ provide OPERATOR_PK (testnet) or CAST_ACCOUNT (+CAST_FROM) for mainnet cast wallet"; usage
fi

set -x
cast send "$VALIDATOR" 'registerPublicKey(bytes32,bytes)' "$NODE_ID" "$PUBKEY" \
  --rpc-url "$RPC_URL" "${AUTH[@]}"
set +x

echo "── post-check ──"
after="$(cast call "$VALIDATOR" 'isRegistered(bytes32)(bool)' "$NODE_ID" --rpc-url "$RPC_URL")"
[ "$after" = "true" ] && echo "✅ registered on-chain (isRegistered=true)" || { echo "❌ still not registered — check tx/operator authorization"; exit 1; }
