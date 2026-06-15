<!-- Created: 2026-06-15 -->
# @aastar/dvt-binding-vector

AirAccount's contribution to the DVT cross-repo golden-vector closure (hub: `YetAnotherAA-Validator#42`, AirAccount `#70`).

## What this proves — and what it is NOT

The DVT design adds an independent BLS co-signer for large operations. For that
to be meaningful, **the operation the user authorized with their passkey must be
the exact same operation the DVT nodes co-sign and the KMS signs** — otherwise a
compromised CA could get the user to authorize op A while DVT/KMS sign op B
(bypassing the #68 payload binding).

This vector makes that invariant (**C1**) executable. It is a known-answer test
of the TA's #68 payload-commitment formula:

```
challenge = SHA-256(nonce || userOpHash)
```

(source of truth: `kms/ta/src/main.rs` `verify_challenge_binding`). The WebAuthn
`challenge` the authenticator signs commits to `userOpHash` — the same hash the
DVT nodes feed to `hashToG2` (BLS) and the KMS signs with secp256k1.

> ❗ This is **NOT** a BLS vector and AirAccount is **NOT** a 4th BLS signer. The
> BLS `hashToG2` / `pkAgg` golden vectors are owned by the node (`#42`), SDK
> (`#63`), and contract (`#110`). This file is the **complementary**
> user-authorization↔userOpHash binding — the part only AirAccount owns.

## Alignment

`u0` / `u1` are the **same canonical `userOpHash` values** as
`airaccount-contract/test/HashToG2Golden.t.sol` (vec1 / vec2). So the
user-authorization binding here and the on-chain BLS hashToG2 vectors refer to
byte-identical operations.

## Run

```bash
node --test        # zero dependencies (node:crypto / node:test)
```

CI goes red if the recomputed commitment ever diverges from the published vector.
