#!/usr/bin/env python3
"""KMS Full API Walkthrough Test with real P-256 ECDSA signatures."""

import json
import os
import sys
import time
import hashlib
import requests
from cryptography.hazmat.primitives.asymmetric import ec, utils
from cryptography.hazmat.primitives import hashes, serialization

HOST = os.environ.get("KMS_HOST", "https://kms1.aastar.io")
API_KEY = os.environ.get("KMS_API_KEY", "kms_bfc3d79efeba419db34c18df8e437e96")
ROUNDS = int(os.environ.get("PERF_ROUNDS", "5"))

HEADERS = {
    "Content-Type": "application/json",
    "x-api-key": API_KEY,
}

passed = 0
failed = 0
results = []


def h(target):
    return {**HEADERS, "x-amz-target": f"TrentService.{target}"}


def gen_p256_keypair():
    private_key = ec.generate_private_key(ec.SECP256R1())
    pub_numbers = private_key.public_key().public_numbers()
    x = pub_numbers.x.to_bytes(32, "big")
    y = pub_numbers.y.to_bytes(32, "big")
    pubkey_hex = "04" + x.hex() + y.hex()
    return private_key, pubkey_hex


def make_assertion(private_key):
    auth_data = bytes.fromhex(
        "49960de5880e8c687434170f6476605b8fe4aeb9a28632c7995cf3ba831d9763"
        "0500000001"
    )
    client_data_hash = os.urandom(32)
    digest = hashlib.sha256(auth_data + client_data_hash).digest()
    der_sig = private_key.sign(digest, ec.ECDSA(utils.Prehashed(hashes.SHA256())))

    # Decode DER → r, s
    r_len = der_sig[3]
    r_bytes = der_sig[4 : 4 + r_len]
    s_start = 4 + r_len + 2
    s_len = der_sig[s_start - 1]
    s_bytes = der_sig[s_start : s_start + s_len]
    r_int = int.from_bytes(r_bytes, "big")
    s_int = int.from_bytes(s_bytes, "big")

    return {
        "AuthenticatorData": auth_data.hex(),
        "ClientDataHash": client_data_hash.hex(),
        "Signature": r_int.to_bytes(32, "big").hex() + s_int.to_bytes(32, "big").hex(),
    }


def test(name, method, url, expected_code, headers=None, json_data=None):
    global passed, failed
    try:
        t0 = time.time()
        if method == "GET":
            r = requests.get(url, headers=headers, timeout=120, allow_redirects=False)
        else:
            r = requests.post(url, headers=headers, json=json_data, timeout=120, allow_redirects=False)
        elapsed = time.time() - t0

        ok = r.status_code == expected_code
        status = "PASS" if ok else "FAIL"
        if ok:
            passed += 1
        else:
            failed += 1

        body = r.text[:200]
        print(f"  {'✅' if ok else '❌'} {status} [{r.status_code}] {elapsed:.3f}s  {name}")
        if not ok:
            print(f"     Expected {expected_code}, body: {body}")
        results.append((name, r.status_code, elapsed, ok))
        return r, elapsed
    except Exception as e:
        failed += 1
        print(f"  ❌ FAIL [ERR] {name}: {e}")
        results.append((name, 0, 0, False))
        return None, 0


def perf_test(name, method, url, headers, json_data, rounds=5):
    """Run multiple rounds and return times."""
    times = []
    for i in range(rounds):
        t0 = time.time()
        if method == "GET":
            r = requests.get(url, headers=headers, timeout=120)
        else:
            r = requests.post(url, headers=headers, json=json_data, timeout=120)
        elapsed = time.time() - t0
        times.append(elapsed)
        if r.status_code != 200:
            print(f"     Round {i+1}: {r.status_code} ERR ({elapsed:.3f}s)")
        else:
            print(f"     Round {i+1}: {elapsed:.3f}s")
    return times


print("=" * 60)
print(" KMS Full API Walkthrough Test")
print(f" Date: {time.strftime('%Y-%m-%d %H:%M UTC', time.gmtime())}")
print(f" Target: {HOST}")
print(" Board: STM32MP157F-DK2 (Cortex-A7 650MHz)")
print("=" * 60)
print()

# Generate P-256 keypair
private_key, pubkey_hex = gen_p256_keypair()
print(f"Generated P-256 keypair: 0x{pubkey_hex[:16]}...")
print()

# ─── FUNCTIONAL TESTS ───
print("─── Functional Tests ───")
print()

# 1. Health
test("Health (GET, no auth)", "GET", f"{HOST}/health", 200)

# 2. ListKeys
r, _ = test("ListKeys", "POST", f"{HOST}/ListKeys", 200, h("ListKeys"), {})
if r:
    keys = r.json().get("Keys", [])
    print(f"     Found {len(keys)} wallets")

# 3. ListKeys without API key
test("ListKeys WITHOUT API key (expect 401)", "POST", f"{HOST}/ListKeys", 401,
     {"Content-Type": "application/json", "x-amz-target": "TrentService.ListKeys"}, {})

# 4. CreateKey
r, create_time = test("CreateKey (real P-256 passkey)", "POST", f"{HOST}/CreateKey", 200,
    h("CreateKey"), {
        "Description": "walkthrough-test",
        "KeyUsage": "SIGN_VERIFY",
        "KeySpec": "ECC_SECG_P256K1",
        "Origin": "TEE_GENERATED",
        "PasskeyPublicKey": pubkey_hex,
    })
wallet_id = ""
if r and r.status_code == 200:
    wallet_id = r.json()["KeyMetadata"]["KeyId"]
    print(f"     Wallet ID: {wallet_id}")

# 5. CreateKey without passkey
test("CreateKey WITHOUT passkey (expect 400)", "POST", f"{HOST}/CreateKey", 400,
    h("CreateKey"), {
        "Description": "no-passkey",
        "KeyUsage": "SIGN_VERIFY",
        "KeySpec": "ECC_SECG_P256K1",
        "Origin": "TEE_GENERATED",
    })

# 6. DescribeKey
if wallet_id:
    r, _ = test("DescribeKey", "POST", f"{HOST}/DescribeKey", 200,
        h("DescribeKey"), {"KeyId": wallet_id})
    if r and r.status_code == 200:
        meta = r.json()["KeyMetadata"]
        print(f"     PasskeyPubKey: {meta.get('PasskeyPublicKey', '?')[:20]}...")

# 7. DescribeKey non-existent
test("DescribeKey non-existent (expect 400)", "POST", f"{HOST}/DescribeKey", 400,
    h("DescribeKey"), {"KeyId": "00000000-0000-0000-0000-000000000000"})

# Wait for background derivation
if wallet_id:
    print()
    print("  ⏳ Waiting 15s for background address derivation...")
    time.sleep(15)
    r = requests.post(f"{HOST}/DescribeKey", headers=h("DescribeKey"),
                      json={"KeyId": wallet_id}, timeout=30)
    addr = r.json().get("KeyMetadata", {}).get("Address", "pending")
    print(f"     Address: {addr}")
    print()

# 8. GetPublicKey with real assertion
if wallet_id:
    assertion = make_assertion(private_key)
    r, _ = test("GetPublicKey (real P-256 assertion)", "POST", f"{HOST}/GetPublicKey", 200,
        h("GetPublicKey"), {
            "KeyId": wallet_id,
            "DerivationPath": "m/44'/60'/0'/0/0",
            "Passkey": assertion,
        })
    if r and r.status_code == 200:
        print(f"     PublicKey: {r.json().get('PublicKey', '?')[:30]}...")

# 9. SignHash with real assertion
if wallet_id:
    assertion = make_assertion(private_key)
    test_hash = "aabbccdd00112233445566778899aabbccddeeff00112233445566778899aabb"
    r, _ = test("SignHash (real P-256 assertion)", "POST", f"{HOST}/SignHash", 200,
        h("SignHash"), {
            "KeyId": wallet_id,
            "SigningAlgorithm": "ECDSA_SHA_256",
            "Hash": test_hash,
            "DerivationPath": "m/44'/60'/0'/0/0",
            "Passkey": assertion,
        })
    if r and r.status_code == 200:
        sig = r.json().get("Signature", "")
        print(f"     Signature: {sig[:30]}... ({len(sig)} chars)")

# 10. Sign message
if wallet_id:
    assertion = make_assertion(private_key)
    r, _ = test("Sign message (real P-256 assertion)", "POST", f"{HOST}/Sign", 200,
        h("Sign"), {
            "KeyId": wallet_id,
            "SigningAlgorithm": "ECDSA_SHA_256",
            "Message": "hello world from walkthrough test",
            "MessageType": "RAW",
            "DerivationPath": "m/44'/60'/0'/0/0",
            "Passkey": assertion,
        })
    if r and r.status_code == 200:
        sig = r.json().get("Signature", "")
        print(f"     Signature: {sig[:30]}... ({len(sig)} chars)")

# 11. Sign transaction (EIP-155)
if wallet_id:
    assertion = make_assertion(private_key)
    tx_hex = "f86c808504a817c80082520894d46e8dd67c5d32be8058bb8eb970870f07244567849184e72a80801ca0d46e8dd67c5d32be8d46e8dd67c5d32be8d46e8dd67c5d32be8d46e8dd67c5a0d46e8dd67c5d32be8d46e8dd67c5d32be8d46e8dd67c5d32be8d46e8dd67c5"
    r, _ = test("Sign transaction EIP-155", "POST", f"{HOST}/Sign", 200,
        h("Sign"), {
            "KeyId": wallet_id,
            "SigningAlgorithm": "ECDSA_SHA_256",
            "Message": tx_hex,
            "MessageType": "TRANSACTION",
            "DerivationPath": "m/44'/60'/0'/0/0",
            "Passkey": assertion,
        })

# 12. SignHash WITHOUT passkey (expect reject)
if wallet_id:
    test("SignHash WITHOUT passkey (expect 400)", "POST", f"{HOST}/SignHash", 400,
        h("SignHash"), {
            "KeyId": wallet_id,
            "SigningAlgorithm": "ECDSA_SHA_256",
            "Hash": test_hash,
            "DerivationPath": "m/44'/60'/0'/0/0",
        })

# 13. SignHash with WRONG key signature
if wallet_id:
    wrong_key, _ = gen_p256_keypair()
    bad_assertion = make_assertion(wrong_key)
    test("SignHash with WRONG passkey (expect 400)", "POST", f"{HOST}/SignHash", 400,
        h("SignHash"), {
            "KeyId": wallet_id,
            "SigningAlgorithm": "ECDSA_SHA_256",
            "Hash": test_hash,
            "DerivationPath": "m/44'/60'/0'/0/0",
            "Passkey": bad_assertion,
        })

print()
print("─── Performance Tests ───")
print()

# Use existing wallet from earlier functional test
if wallet_id:
    # Health perf
    print(f"  Health ({ROUNDS} rounds):")
    health_times = perf_test("Health", "GET", f"{HOST}/health", {}, None, ROUNDS)

    print(f"  ListKeys ({ROUNDS} rounds):")
    list_times = perf_test("ListKeys", "POST", f"{HOST}/ListKeys", h("ListKeys"), {}, ROUNDS)

    print(f"  DescribeKey ({ROUNDS} rounds):")
    desc_times = perf_test("DescribeKey", "POST", f"{HOST}/DescribeKey",
        h("DescribeKey"), {"KeyId": wallet_id}, ROUNDS)

    print(f"  GetPublicKey ({ROUNDS} rounds):")
    gpk_times = []
    for i in range(ROUNDS):
        assertion = make_assertion(private_key)
        t0 = time.time()
        r = requests.post(f"{HOST}/GetPublicKey", headers=h("GetPublicKey"),
            json={"KeyId": wallet_id, "DerivationPath": "m/44'/60'/0'/0/0", "Passkey": assertion}, timeout=120)
        elapsed = time.time() - t0
        gpk_times.append(elapsed)
        print(f"     Round {i+1}: {elapsed:.3f}s {'OK' if r.status_code==200 else 'ERR '+str(r.status_code)}")

    print(f"  SignHash ({ROUNDS} rounds):")
    sh_times = []
    for i in range(ROUNDS):
        assertion = make_assertion(private_key)
        t0 = time.time()
        r = requests.post(f"{HOST}/SignHash", headers=h("SignHash"),
            json={"KeyId": wallet_id, "SigningAlgorithm": "ECDSA_SHA_256",
                  "Hash": "aabbccdd00112233445566778899aabbccddeeff00112233445566778899aabb",
                  "DerivationPath": "m/44'/60'/0'/0/0", "Passkey": assertion}, timeout=120)
        elapsed = time.time() - t0
        sh_times.append(elapsed)
        print(f"     Round {i+1}: {elapsed:.3f}s {'OK' if r.status_code==200 else 'ERR '+str(r.status_code)}")

    print(f"  Sign message ({ROUNDS} rounds):")
    sm_times = []
    for i in range(ROUNDS):
        assertion = make_assertion(private_key)
        t0 = time.time()
        r = requests.post(f"{HOST}/Sign", headers=h("Sign"),
            json={"KeyId": wallet_id, "SigningAlgorithm": "ECDSA_SHA_256",
                  "Message": "perf test message", "MessageType": "RAW",
                  "DerivationPath": "m/44'/60'/0'/0/0", "Passkey": assertion}, timeout=120)
        elapsed = time.time() - t0
        sm_times.append(elapsed)
        print(f"     Round {i+1}: {elapsed:.3f}s {'OK' if r.status_code==200 else 'ERR '+str(r.status_code)}")

    print(f"  Sign transaction ({ROUNDS} rounds):")
    st_times = []
    for i in range(ROUNDS):
        assertion = make_assertion(private_key)
        t0 = time.time()
        r = requests.post(f"{HOST}/Sign", headers=h("Sign"),
            json={"KeyId": wallet_id, "SigningAlgorithm": "ECDSA_SHA_256",
                  "Message": tx_hex, "MessageType": "TRANSACTION",
                  "DerivationPath": "m/44'/60'/0'/0/0", "Passkey": assertion}, timeout=120)
        elapsed = time.time() - t0
        st_times.append(elapsed)
        print(f"     Round {i+1}: {elapsed:.3f}s {'OK' if r.status_code==200 else 'ERR '+str(r.status_code)}")

    print()
    print("─── Performance Summary ───")
    print()

    def stats(name, times):
        if not times:
            return
        avg = sum(times) / len(times)
        mn = min(times)
        mx = max(times)
        med = sorted(times)[len(times) // 2]
        print(f"  {name:25s}  avg={avg:.3f}s  min={mn:.3f}s  med={med:.3f}s  max={mx:.3f}s")
        return {"avg": round(avg * 1000), "min": round(mn * 1000),
                "med": round(med * 1000), "max": round(mx * 1000)}

    perf_data = {}
    perf_data["Health"] = stats("Health", health_times)
    perf_data["ListKeys"] = stats("ListKeys", list_times)
    perf_data["DescribeKey"] = stats("DescribeKey", desc_times)
    perf_data["GetPublicKey"] = stats("GetPublicKey", gpk_times)
    perf_data["SignHash"] = stats("SignHash", sh_times)
    perf_data["Sign (message)"] = stats("Sign (message)", sm_times)
    perf_data["Sign (transaction)"] = stats("Sign (transaction)", st_times)
    perf_data["CreateKey"] = {"avg": round(create_time * 1000), "min": "-", "med": "-", "max": "-"}
    print(f"  {'CreateKey':25s}  single={create_time:.3f}s (excludes background derivation ~90s)")

    # Write perf JSON for doc update
    with open("/tmp/kms_perf_results.json", "w") as f:
        json.dump(perf_data, f, indent=2)

print()
print("=" * 60)
print(f" Results: {passed} passed, {failed} failed, {passed + failed} total")
print("=" * 60)
