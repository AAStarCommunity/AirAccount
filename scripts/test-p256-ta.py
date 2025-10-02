#!/usr/bin/env python3
"""
P-256 ECDSA signature verification test script
Generates test vectors and tests TA P-256 verification
"""

from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.backends import default_backend
import subprocess
import json

def main():
    print("🔐 P-256 ECDSA Test Vector Generation\n")

    # 1. Generate P-256 keypair
    private_key = ec.generate_private_key(ec.SECP256R1(), default_backend())
    public_key = private_key.public_key()

    # 2. Export public key (SEC1 uncompressed format: 0x04 + x + y)
    pubkey_sec1 = public_key.public_bytes(
        encoding=serialization.Encoding.X962,
        format=serialization.PublicFormat.UncompressedPoint
    )
    print(f"📌 Public Key (SEC1, {len(pubkey_sec1)} bytes):")
    print(f"   {pubkey_sec1.hex()}\n")

    # 3. Test message
    message = b"Passkey Challenge: 0xabcdef1234567890"
    print(f"📝 Test Message:")
    print(f"   {message.decode()}\n")

    # 4. Sign message
    signature_der = private_key.sign(
        message,
        ec.ECDSA(hashes.SHA256())
    )
    print(f"✍️  Signature (DER, {len(signature_der)} bytes):")
    print(f"   {signature_der.hex()}\n")

    # 5. Verify locally (sanity check)
    try:
        public_key.verify(signature_der, message, ec.ECDSA(hashes.SHA256()))
        print("✅ Local verification: SUCCESS\n")
    except:
        print("❌ Local verification: FAILED\n")
        return

    # 6. Export test vectors
    test_vectors = {
        "pubkey_sec1": pubkey_sec1.hex(),
        "message": message.hex(),
        "signature_der": signature_der.hex()
    }

    print("📦 Test Vectors (JSON):")
    print(json.dumps(test_vectors, indent=2))
    print()

    # 7. Save test vectors for TA testing
    with open("/tmp/p256_test_vectors.json", "w") as f:
        json.dump(test_vectors, f, indent=2)

    print("💾 Test vectors saved to: /tmp/p256_test_vectors.json")
    print()
    print("🚀 Next step: Build and test in OP-TEE:")
    print("   ./scripts/kms-dev-cycle.sh")

if __name__ == "__main__":
    main()
