#!/bin/bash
# Extract the attestation measurement (shdr::hash) from a signed OP-TEE TA.
#
# This is the value the OP-TEE attestation PTA returns as `ta_measurement`
# (GET_TA_SHDR_DIGEST) and that `@aastar/attestation-verifier` compares against
# (issue #37). It is the SHA-256 stored in the TA's signed header — computed over
# the TA payload, NOT the signature, so it is deterministic across rebuilds even
# though the PSS signature (random salt) differs every time.
#
# Anyone can run this on their own reproducible build of the TA and confirm it
# equals the value the deployed KMS reports via GET /attestation — i.e. verify
# "the running TA is built from this exact public source" WITHOUT trusting AAStar
# (reference-value distribution tier 3, design doc §7.1 / R-4).
#
# Usage:
#   ./scripts/ta-measurement.sh <path-to-signed.ta>
#   ./scripts/ta-measurement.sh build/mx93/4319f351-0b24-4097-b659-80ee4f824cdd.ta
#
# Compare against the device:
#   NONCE=$(python3 -c 'import secrets;print(secrets.token_hex(32))')
#   curl -s "https://kms.aastar.io/attestation?nonce=$NONCE" | python3 -c \
#     'import sys,json;print(json.load(sys.stdin)["ta_measurement"])'

set -euo pipefail

TA="${1:-}"
[[ -n "$TA" && -f "$TA" ]] || { echo "usage: $0 <path-to-signed.ta>" >&2; exit 1; }

# OP-TEE signed-header layout (struct shdr, little-endian):
#   u32 magic (0x4f545348 'HSTO') | u32 img_type | u32 img_size | u32 algo
#   u16 hash_size | u16 sig_size | u8 hash[hash_size] | u8 sig[sig_size]
# shdr::hash therefore starts at byte offset 20.
python3 - "$TA" <<'PY'
import struct, sys
ta = open(sys.argv[1], "rb").read()
if len(ta) < 20:
    sys.exit("file too short to be a signed TA")
magic, img_type, img_size, algo, hash_size, sig_size = struct.unpack("<IIIIHH", ta[:20])
if magic != 0x4f545348:
    sys.exit(f"not an OP-TEE signed TA (magic=0x{magic:08x}, expected 0x4f545348)")
h = ta[20:20 + hash_size]
if len(h) != hash_size:
    sys.exit("truncated header: hash field incomplete")
print(h.hex())
PY
