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

from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature, Prehashed


def gen_keypair():
    """Generate P-256 keypair, return dict with hex pubkey and PEM privkey."""
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
    rp_id_hash = hashlib.sha256(b"localhost").digest()
    flags = bytes([0x05])  # UP=1, UV=1
    sign_count = struct.pack(">I", 1)
    auth_data = rp_id_hash + flags + sign_count

    # clientDataJSON hash
    client_data_json = json.dumps({
        "type": "webauthn.get",
        "challenge": "dGVzdC1jaGFsbGVuZ2U",
        "origin": "http://localhost:3000"
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


def generate_fixture(output_path: str, label: str = "test-user"):
    """Generate a complete test user fixture file."""
    kp = gen_keypair()
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
    return fixture


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


def gen_all():
    """Generate all test fixtures in test-fixtures/ directory."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    fixtures_dir = os.path.join(script_dir, "test-fixtures")
    os.makedirs(fixtures_dir, exist_ok=True)

    for i in range(1, 4):
        generate_fixture(
            os.path.join(fixtures_dir, f"user{i}.json"),
            f"test-user-{i}"
        )

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
    elif cmd == "fixture":
        path = sys.argv[2] if len(sys.argv) > 2 else "/dev/stdout"
        label = sys.argv[3] if len(sys.argv) > 3 else "test-user"
        generate_fixture(path, label)
    elif cmd == "gen-all":
        gen_all()
    else:
        print("Usage: p256_helper.py gen | assertion <pem> | fixture <path> [label] | gen-all")
