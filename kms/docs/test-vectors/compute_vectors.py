#!/usr/bin/env python3
"""Runnable cross-check for KMS grant-session commitment vectors (#119).

Recomputes the grant `final_hash` (and commitment) from the documented byte layout
and asserts it matches commitment-vectors.json. The layout mirrors the TA
(kms/ta/src/main.rs build_grant_session_inner / build_p256_grant_session_inner +
keccak_packed_addresses/keccak_packed_selectors) AND the on-chain
SessionKeyValidator hash. Run after any change to those TA functions.

  pip install pycryptodome      # keccak256 (Ethereum), NOT hashlib.sha3
  python3 compute_vectors.py    # exits non-zero on mismatch

Mint ops are nonce-only (no commitment) — see commitment-vectors.json _mint_note.
"""
import base64
import hashlib
import json
import os
import sys
import uuid

try:
    from Crypto.Hash import keccak
except ImportError:
    sys.exit("need pycryptodome: pip install pycryptodome")


def k256(b: bytes) -> bytes:
    h = keccak.new(digest_bits=256)
    h.update(b)
    return h.digest()


def u256(n: int) -> bytes:
    return n.to_bytes(32, "big")


def addr32(a: str) -> bytes:  # address: left-pad to 32 (right-aligned)
    return b"\x00" * 12 + bytes.fromhex(a[2:])


def sel32(s: str) -> bytes:  # bytes4: right-pad to 32 (left-aligned)
    return bytes.fromhex(s[2:]) + b"\x00" * 28


def keccak_packed_addresses(addrs):  # Solidity abi.encodePacked(address[]) — pad-32 each
    return k256(b"".join(addr32(a) for a in addrs))


def keccak_packed_selectors(sels):  # Solidity abi.encodePacked(bytes4[]) — pad-32 each
    return k256(b"".join(sel32(s) for s in sels))


def eip191(inner: bytes) -> bytes:
    return k256(b"\x19Ethereum Signed Message:\n32" + inner)


def grant_inner(inp, variant):
    cth = keccak_packed_addresses(inp["callTargets"])
    sth = keccak_packed_selectors(inp["selectorAllowlist"])
    chain, vc, acct = inp["chainId"], inp["verifyingContract"], inp["account"]
    expiry, cscope, sscope = inp["expiry"], inp["contractScope"], inp["selectorScope"]
    vl, vw = inp["velocityLimit"], inp["velocityWindow"]
    nonce = bytes.fromhex(inp["grant_nonce_hex"])
    head = u256(chain) + addr32(vc) + addr32(acct)
    if variant["primary_type"] == "GRANT_SESSION_V2":
        tag = b"GRANT_SESSION_V2"
        head += addr32(variant["sessionKey"])
        n_args = 13
    else:
        tag = b"GRANT_P256_SESSION_V2"
        head += bytes.fromhex(variant["keyX_hex"]) + bytes.fromhex(variant["keyY_hex"])
        n_args = 14
    head += u256(expiry) + addr32(cscope) + sel32(sscope) + u256(vl) + u256(vw)
    head += cth + sth + nonce
    str_off = n_args * 32
    buf = u256(str_off) + head + u256(len(tag)) + tag
    buf += b"\x00" * ((32 - len(tag) % 32) % 32)  # pad string to 32-byte word
    return k256(buf)


def b64u(b: bytes) -> str:
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode()


def mint_label_digest(tag: str, wallet_id: str, label: str) -> bytes:
    h = hashlib.sha256()
    h.update(tag.encode())
    h.update(uuid.UUID(wallet_id).bytes)            # 16 bytes big-endian
    h.update(hashlib.sha256(label.encode()).digest())
    return h.digest()


def agent_refresh_digest(tag: str, wallet_id: str, agent_index: int) -> bytes:
    h = hashlib.sha256()
    h.update(tag.encode())
    h.update(uuid.UUID(wallet_id).bytes)
    h.update(agent_index.to_bytes(4, "big"))
    return h.digest()


def main():
    path = os.path.join(os.path.dirname(__file__), "commitment-vectors.json")
    vec = json.load(open(path))
    fails = 0

    # ── mint (#115): commitment = SHA-256(nonce || SHA-256(tag || wallet_id || SHA-256(label)))
    mi = vec["mint"]["input"]
    nonce = bytes.fromhex(mi["nonce_hex"])
    for k, v in vec["mint"].items():
        if k.startswith("_") or k == "input":
            continue
        if v.get("is_refresh"):
            d = agent_refresh_digest(v["tag"], mi["wallet_id"], v["agent_index"])
        else:
            d = mint_label_digest(v["tag"], mi["wallet_id"], v["label"])
        ok_d = d.hex() == v["mint_digest_hex"]
        c = b64u(hashlib.sha256(nonce + d).digest())
        ok_c = c == v["commitment_b64url"]
        fails += (not ok_d) + (not ok_c)
        print(f"[{'PASS' if ok_d and ok_c else 'FAIL'}] mint.{k} (digest+commitment)")
        if not (ok_d and ok_c):
            print(f"        digest got {d.hex()} want {v['mint_digest_hex']}")

    for case_name, case in vec["grant"].items():
        if case_name.startswith("_"):
            continue
        inp = case["input"]
        for v in ("secp256k1", "p256"):
            variant = case[v]
            got = eip191(grant_inner(inp, variant)).hex()
            want = variant["final_hash_hex"]
            ok = got == want
            fails += not ok
            print(f"[{'PASS' if ok else 'FAIL'}] grant.{case_name}.{v} final_hash")
            if not ok:
                print(f"        got  {got}\n        want {want}")
            elif "commitment_b64url" in variant and "challenge_nonce_hex" in inp:
                cn = bytes.fromhex(inp["challenge_nonce_hex"])
                fh = bytes.fromhex(want)
                c = base64.urlsafe_b64encode(hashlib.sha256(cn + fh).digest()).rstrip(b"=").decode()
                cok = c == variant["commitment_b64url"]
                fails += not cok
                print(f"[{'PASS' if cok else 'FAIL'}] grant.{case_name}.{v} commitment")
    print(f"\n{'ALL PASS' if not fails else str(fails) + ' FAILED'}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
