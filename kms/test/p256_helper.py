#!/usr/bin/env python3
"""P-256 helper: generate keys, create passkey assertions, and manage test fixtures for KMS.

Dependencies: pip3 install cryptography

Usage:
  python3 p256_helper.py gen                            # Generate keypair JSON
  python3 p256_helper.py assertion <privkey_pem>        # Create signed assertion
  python3 p256_helper.py fixture <output.json> [label]  # Generate full test user fixture
  python3 p256_helper.py gen-all                        # Generate user1..user3 + transactions
"""

import sys
import json
import hashlib
import os
import struct
import base64

from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature, Prehashed


def gen_keypair(d_hex: str = None):
    """P-256 keypair. If d_hex (32-byte scalar) is given, derive deterministically;
    otherwise generate a fresh random key. Returns hex pubkey, PEM privkey, and the
    private scalar (hex) so callers can persist it outside git (.env.kms-test)."""
    if d_hex:
        private_key = ec.derive_private_key(int(d_hex, 16), ec.SECP256R1())
    else:
        private_key = ec.generate_private_key(ec.SECP256R1())
    pub_bytes = private_key.public_key().public_bytes(
        serialization.Encoding.X962,
        serialization.PublicFormat.UncompressedPoint
    )
    pem = private_key.private_bytes(
        serialization.Encoding.PEM,
        serialization.PrivateFormat.PKCS8,
        serialization.NoEncryption()
    ).decode()
    return {
        "public_key_hex": "0x" + pub_bytes.hex(),
        "private_key_pem": pem,
        "priv_d_hex": format(private_key.private_numbers().private_value, "064x"),
    }


def make_assertion(privkey_pem: str):
    """Create a valid passkey assertion signed with the given P-256 private key.

    TA verification: digest = SHA256(auth_data || client_data_hash), then ECDSA verify on digest.
    CA verification: verify(auth_data || cdh, sig) — internally hashes with SHA-256.
    Both see SHA256(auth_data || cdh) — single hash.
    We sign the pre-computed digest using Prehashed.
    """
    private_key = serialization.load_pem_private_key(privkey_pem.encode(), password=None)

    # Realistic authenticatorData: rpIdHash(32) + flags(1) + signCount(4) = 37 bytes
    # rpId must match the TA's hardcoded EXPECTED_RP_ID_HASH = SHA-256("aastar.io")
    # (PR#44 / Issue #39). Using any other rpId makes the TA reject the assertion
    # with "WebAuthn rpId hash mismatch".
    rp_id_hash = hashlib.sha256(b"aastar.io").digest()
    flags = bytes([0x05])  # UP=1, UV=1
    sign_count = struct.pack(">I", 1)
    auth_data = rp_id_hash + flags + sign_count

    # clientDataJSON hash
    client_data_json = json.dumps({
        "type": "webauthn.get",
        "challenge": "dGVzdC1jaGFsbGVuZ2U",
        "origin": "https://aastar.io"
    }).encode()
    client_data_hash = hashlib.sha256(client_data_json).digest()

    # digest = SHA256(auth_data || client_data_hash)
    msg = auth_data + client_data_hash
    digest = hashlib.sha256(msg).digest()

    # Sign digest directly (prehashed) — ECDSA operates on digest without re-hashing
    sig_der = private_key.sign(digest, ec.ECDSA(Prehashed(hashes.SHA256())))

    # Decode DER to r,s and encode as hex
    r, s = decode_dss_signature(sig_der)

    return {
        "authenticator_data": auth_data.hex(),
        "client_data_hash": client_data_hash.hex(),
        "signature_r": r.to_bytes(32, 'big').hex(),
        "signature_s": s.to_bytes(32, 'big').hex(),
    }


def _b64url(b: bytes) -> str:
    """base64url without padding (WebAuthn wire format)."""
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode()


def make_ceremony_assertion(privkey_pem: str, challenge_b64url: str, credential_id: str = "dGVzdC1jcmVkZW50aWFs", sign_count: int = 1):
    """Create a WebAuthn AuthenticationResponseJSON for a DYNAMIC challenge.

    Used to test ceremony-based endpoints (agent key / grant session / p256
    session / SignTypedData) which require a real begin-authentication challenge,
    unlike the legacy fixed-challenge path of make_assertion().

    The challenge MUST be the exact base64url string returned by
    BeginAuthentication. clientDataJSON embeds it; the CA decodes clientDataJSON,
    checks challenge == stored, verifies rpIdHash, then ECDSA-verifies
    signature over (authenticatorData || SHA256(clientDataJSON)).
    """
    private_key = serialization.load_pem_private_key(privkey_pem.encode(), password=None)

    # clientDataJSON with the dynamic challenge (compact, no spaces)
    client_data_json = json.dumps({
        "type": "webauthn.get",
        "challenge": challenge_b64url,
        "origin": "https://aastar.io",
    }, separators=(",", ":")).encode()
    client_data_hash = hashlib.sha256(client_data_json).digest()

    # authenticatorData: rpIdHash("aastar.io") + flags(UP|UV) + signCount
    # signCount must strictly increase across ceremonies for the same wallet
    # (anti-clone check), so callers pass an incrementing value.
    rp_id_hash = hashlib.sha256(b"aastar.io").digest()
    auth_data = rp_id_hash + bytes([0x05]) + struct.pack(">I", sign_count)

    # signature over (authData || clientDataHash), DER; CA hashes with SHA-256
    digest = hashlib.sha256(auth_data + client_data_hash).digest()
    sig_der = private_key.sign(digest, ec.ECDSA(Prehashed(hashes.SHA256())))

    return {
        "id": credential_id,
        "rawId": credential_id,
        "type": "public-key",
        "response": {
            "clientDataJSON": _b64url(client_data_json),
            "authenticatorData": _b64url(auth_data),
            "signature": _b64url(sig_der),
        },
    }


def _cbor_uint(n: int) -> bytes:
    """CBOR encode a small unsigned/negative int (sufficient for COSE labels)."""
    if n >= 0:
        if n < 24:
            return bytes([n])
        if n < 256:
            return bytes([0x18, n])
        raise ValueError("uint too large for minimal encoder")
    # negative: major type 1, value = -1-n
    v = -1 - n
    if v < 24:
        return bytes([0x20 | v])
    if v < 256:
        return bytes([0x38, v])
    raise ValueError("neg too large for minimal encoder")


def _cbor_bytes(b: bytes) -> bytes:
    n = len(b)
    if n < 24:
        return bytes([0x40 | n]) + b
    if n < 256:
        return bytes([0x58, n]) + b
    return bytes([0x59, n >> 8, n & 0xFF]) + b


def _cbor_text(s: str) -> bytes:
    b = s.encode()
    n = len(b)
    if n < 24:
        return bytes([0x60 | n]) + b
    return bytes([0x78, n]) + b


def make_registration_response(privkey_pem: str, challenge_b64url: str, credential_id: str = "dGVzdC1jcmVkZW50aWFs"):
    """Build a WebAuthn RegistrationResponseJSON with 'none' attestation.

    CompleteRegistration does NOT verify the attestation statement signature — it
    only checks clientDataJSON (type/challenge/origin), rpIdHash, UP+AT flags, and
    parses the COSE P-256 public key out of authData. So a hand-built 'none'
    attestation with a real P-256 key (matching the fixture) registers cleanly.
    """
    private_key = serialization.load_pem_private_key(privkey_pem.encode(), password=None)
    nums = private_key.public_key().public_numbers()
    x = nums.x.to_bytes(32, "big")
    y = nums.y.to_bytes(32, "big")

    client_data_json = json.dumps({
        "type": "webauthn.create",
        "challenge": challenge_b64url,
        "origin": "https://aastar.io",
    }, separators=(",", ":")).encode()

    # COSE_Key: {1:2(EC2), 3:-7(ES256), -1:1(P-256), -2:x, -3:y}
    cose = (bytes([0xA5])
            + _cbor_uint(1) + _cbor_uint(2)
            + _cbor_uint(3) + _cbor_uint(-7)
            + _cbor_uint(-1) + _cbor_uint(1)
            + _cbor_uint(-2) + _cbor_bytes(x)
            + _cbor_uint(-3) + _cbor_bytes(y))

    cred_id = base64.urlsafe_b64decode(credential_id + "=" * (-len(credential_id) % 4))
    rp_id_hash = hashlib.sha256(b"aastar.io").digest()
    flags = bytes([0x41])           # UP(0x01) | AT(0x40)
    sign_count = struct.pack(">I", 0)
    aaguid = b"\x00" * 16
    cred_id_len = struct.pack(">H", len(cred_id))
    auth_data = rp_id_hash + flags + sign_count + aaguid + cred_id_len + cred_id + cose

    # attestationObject: {"fmt":"none","attStmt":{},"authData":bytes}  (CBOR map, 3 keys)
    att_obj = (bytes([0xA3])
               + _cbor_text("fmt") + _cbor_text("none")
               + _cbor_text("attStmt") + bytes([0xA0])
               + _cbor_text("authData") + _cbor_bytes(auth_data))

    return {
        "id": credential_id,
        "rawId": credential_id,
        "type": "public-key",
        "response": {
            "clientDataJSON": _b64url(client_data_json),
            "attestationObject": _b64url(att_obj),
        },
        "clientExtensionResults": {},
    }


def generate_fixture(output_path: str, label: str = "test-user", d_hex: str = None):
    """Generate a complete test user fixture file (optionally from a fixed scalar)."""
    kp = gen_keypair(d_hex)
    assertion = make_assertion(kp["private_key_pem"])

    fixture = {
        "label": label,
        "public_key_hex": kp["public_key_hex"],
        "private_key_pem": kp["private_key_pem"],
        "sample_assertion": assertion,
    }

    with open(output_path, 'w') as f:
        json.dump(fixture, f, indent=2)
    print(f"Generated: {output_path} ({label})")
    return kp


def generate_transactions():
    """Generate realistic EIP-155 transaction templates."""
    return [
        {
            "label": "ETH transfer",
            "chain_id": 1,
            "nonce": 0,
            "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f2bD18",
            "value": 1000000000000000000,
            "gas_price": 20000000000,
            "gas_limit": 21000,
        },
        {
            "label": "ERC20 approve",
            "chain_id": 1,
            "nonce": 1,
            "to": "0xdAC17F958D2ee523a2206206994597C13D831ec7",
            "value": 0,
            "gas_price": 30000000000,
            "gas_limit": 60000,
        },
        {
            "label": "Sepolia test tx",
            "chain_id": 11155111,
            "nonce": 5,
            "to": "0x0000000000000000000000000000000000000001",
            "value": 100000000000000,
            "gas_price": 10000000000,
            "gas_limit": 21000,
        },
    ]


def _env_path():
    """Path to the git-ignored keystore that holds the test private scalars."""
    return os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                        "..", ".env.kms-test")


def _load_env_scalars():
    """Read TEST_P256_<n>_PRIV_D_HEX from .env.kms-test (returns {n: scalar_hex})."""
    path = _env_path()
    scalars = {}
    if os.path.exists(path):
        for line in open(path):
            line = line.strip()
            if line.startswith("#") or "=" not in line:
                continue
            k, v = line.split("=", 1)
            m = k.strip()
            if m.startswith("TEST_P256_") and m.endswith("_PRIV_D_HEX"):
                n = m[len("TEST_P256_"):-len("_PRIV_D_HEX")]
                d = v.strip().strip('"').removeprefix("0x")
                if len(d) == 64:
                    scalars[n] = d
    return scalars


def _save_env_scalars(keys: dict):
    """Persist scalars+pubkeys to the git-ignored .env.kms-test (single source of truth)."""
    path = _env_path()
    lines = [
        "# AirAccount KMS test keypairs — P-256 WebAuthn passkeys for E2E.",
        "# WARNING: contains EC private key scalars. Git-ignored; NEVER commit.",
        "# Regenerate fixtures from here:  python3 kms/test/p256_helper.py gen-all",
        "",
    ]
    for n in sorted(keys):
        kp = keys[n]
        lines += [
            f"# Key {n} — P256 (WebAuthn passkey for test account {n})",
            f"TEST_P256_{n}_PRIV_D_HEX={kp['priv_d_hex']}",
            f"TEST_P256_{n}_PUB_HEX={kp['public_key_hex']}",
            "",
        ]
    with open(path, "w") as f:
        f.write("\n".join(lines))
    print(f"Wrote keystore: {os.path.normpath(path)} ({len(keys)} keys)")


def gen_all():
    """Materialize test fixtures in test-fixtures/ from .env.kms-test (the git-ignored
    keystore). Reuses existing scalars when present (reproducible); generates fresh keys
    and writes them back when absent. Private keys live ONLY in .env.kms-test, never git."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    fixtures_dir = os.path.join(script_dir, "test-fixtures")
    os.makedirs(fixtures_dir, exist_ok=True)

    env_scalars = _load_env_scalars()
    keys = {}
    for i in range(1, 4):
        n = str(i)
        kp = generate_fixture(
            os.path.join(fixtures_dir, f"user{i}.json"),
            f"test-user-{i}",
            d_hex=env_scalars.get(n),  # None → fresh random
        )
        keys[n] = kp
    _save_env_scalars(keys)  # persist (back-fill any freshly generated)

    txs = generate_transactions()
    tx_path = os.path.join(fixtures_dir, "transactions.json")
    with open(tx_path, 'w') as f:
        json.dump(txs, f, indent=2)
    print(f"Generated: {tx_path} ({len(txs)} transactions)")


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "help"

    if cmd == "gen":
        print(json.dumps(gen_keypair(), indent=2))
    elif cmd == "assertion":
        if len(sys.argv) < 3:
            print("Usage: p256_helper.py assertion <privkey_pem>", file=sys.stderr)
            sys.exit(1)
        pem = sys.argv[2]
        print(json.dumps(make_assertion(pem), indent=2))
    elif cmd == "ceremony":
        # ceremony <pem> <challenge_b64url> [credential_id]
        # → WebAuthn AuthenticationResponseJSON for a dynamic challenge
        if len(sys.argv) < 4:
            print("Usage: p256_helper.py ceremony <privkey_pem> <challenge_b64url> [credential_id]", file=sys.stderr)
            sys.exit(1)
        pem = sys.argv[2]
        challenge = sys.argv[3]
        cred_id = sys.argv[4] if len(sys.argv) > 4 else "dGVzdC1jcmVkZW50aWFs"
        sc = int(sys.argv[5]) if len(sys.argv) > 5 else 1
        print(json.dumps(make_ceremony_assertion(pem, challenge, cred_id, sc)))
    elif cmd == "registration":
        # registration <pem> <challenge_b64url> [credential_id]
        # → WebAuthn RegistrationResponseJSON ('none' attestation) for CompleteRegistration
        if len(sys.argv) < 4:
            print("Usage: p256_helper.py registration <privkey_pem> <challenge_b64url> [credential_id]", file=sys.stderr)
            sys.exit(1)
        pem = sys.argv[2]
        challenge = sys.argv[3]
        cred_id = sys.argv[4] if len(sys.argv) > 4 else "dGVzdC1jcmVkZW50aWFs"
        print(json.dumps(make_registration_response(pem, challenge, cred_id)))
    elif cmd == "fixture":
        path = sys.argv[2] if len(sys.argv) > 2 else "/dev/stdout"
        label = sys.argv[3] if len(sys.argv) > 3 else "test-user"
        generate_fixture(path, label)
    elif cmd == "gen-all":
        gen_all()
    else:
        print("Usage: p256_helper.py gen | assertion <pem> | ceremony <pem> <challenge_b64url> [cred_id] | fixture <path> [label] | gen-all")
