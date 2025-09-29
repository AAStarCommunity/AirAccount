#!/usr/bin/env python3
"""
Verify the cryptographic signature from our KMS service
"""
import base64
import hashlib
from ecdsa import VerifyingKey, SECP256k1
from ecdsa.util import sigdecode_der

def verify_signature():
    # Data from our tests
    public_key_b64 = "AgmW/E0W9kPa5NMwAzjCJ3mBeiLC8kCVO45M7xBWCcwT"
    signature_b64 = "3Zcuv9+vQp2aTmxDLkLcT2Y+MG1FQo2+dAC5/DsnugQ2gExaTC/AqpEW3Aci8oOxecXhqdKweBMmvjfNIwqOHA=="
    message_b64 = "SGVsbG8gV29ybGQ="  # "Hello World" in base64

    # Decode the data
    public_key_bytes = base64.b64decode(public_key_b64)
    signature_bytes = base64.b64decode(signature_b64)
    message_bytes = base64.b64decode(message_b64)

    print(f"Original message: {message_bytes.decode('utf-8')}")
    print(f"Public key length: {len(public_key_bytes)} bytes")
    print(f"Signature length: {len(signature_bytes)} bytes")

    # Hash the message (SHA3-256 as our KMS does)
    import hashlib
    hasher = hashlib.sha3_256()
    hasher.update(message_bytes)
    message_hash = hasher.digest()

    print(f"Message hash: {message_hash.hex()}")

    # Parse the public key (33 bytes compressed format)
    try:
        # Convert secp256k1 compressed public key to ecdsa format
        from ecdsa.ellipticcurve import Point
        from ecdsa.curves import SECP256k1

        # Parse compressed public key
        if public_key_bytes[0] == 0x02 or public_key_bytes[0] == 0x03:
            # Compressed format - use secp256k1 curve parameters
            p = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
            x = int.from_bytes(public_key_bytes[1:], 'big')
            y_squared = (pow(x, 3, p) + 7) % p
            y = pow(y_squared, (p + 1) // 4, p)

            if public_key_bytes[0] == 0x03 and y % 2 == 0:
                y = p - y
            elif public_key_bytes[0] == 0x02 and y % 2 == 1:
                y = p - y

            point = Point(SECP256k1.curve, x, y)
            vk = VerifyingKey.from_public_point(point, curve=SECP256k1)

            # Parse signature (64 bytes: 32 bytes r + 32 bytes s)
            if len(signature_bytes) == 64:
                r = int.from_bytes(signature_bytes[:32], 'big')
                s = int.from_bytes(signature_bytes[32:], 'big')

                # Verify signature
                try:
                    is_valid = vk.verify_digest(signature_bytes, message_hash,
                                              sigdecode=lambda rs, order: (r, s))
                    print(f"Signature verification: {'✅ VALID' if is_valid else '❌ INVALID'}")
                    return is_valid
                except Exception as e:
                    print(f"Signature verification failed: {e}")
                    return False
            else:
                print(f"Invalid signature length: {len(signature_bytes)} (expected 64)")
                return False
        else:
            print(f"Unsupported public key format: {public_key_bytes[0]:02x}")
            return False

    except Exception as e:
        print(f"Error verifying signature: {e}")
        return False

if __name__ == "__main__":
    verify_signature()