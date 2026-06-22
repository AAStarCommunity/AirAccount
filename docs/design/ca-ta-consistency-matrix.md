<!-- Created: 2026-06-22 -->
# CA/TA challenge-binding consistency matrix

**Why this exists.** Twice (#110, #121) the same bug shipped: the TA's
`verify_passkey_for_wallet(.., payload)` was changed to bind `Some(payload)` for an op,
but the host's `resolve_passkey_assertion(.., delegate)` still passed `delegate=false`,
so the host rejected the `SHA-256(nonce‖payload)` commitment as "not the bare nonce"
**before it reached the TA** — making the TA binding unreachable. memory notes did not
prevent it. This matrix + the gate `scripts/ca-ta-consistency.py` make it a hard,
executable pre-PR check.

## Invariant

For an op carrying a passkey assertion host→TA:

- TA binds **`Some(payload)`** (payload commitment) ⟺ host **`delegate=true`**
  (host skips the challenge-value check, delegates it to the TA; still verifies
  signature + origin + rpId + one-time challenge consume).
- TA binds **`None`** (nonce-only) ⟹ host `delegate=false` (host enforces
  `challenge == bare nonce`). `delegate=true` also works for a None op (a bare nonce
  passes the value check anyway), so None+true is allowed but None+false is the norm.
- **Host-only ops** that never reach the TA (e.g. `UnfreezeKey`) MUST be
  `delegate=false` (the host is the sole gate — no TA backstop).

## Matrix (authoritative)

| op (host fn → TA fn) | TA payload | host delegate | client signs |
|---|---|---|---|
| Sign → sign_transaction | Some(tx_hash) | true | SHA-256(nonce‖tx_hash) |
| SignMessage → sign_message | Some(msg_hash) | true | commitment |
| SignHash → sign_hash | Some(hash) | true | commitment |
| SignTypedData (+voucher/GToken/x402) → sign_typed_data | Some(eip712_digest) | true | commitment |
| SignGrantSession → sign_grant_session | Some(final_hash) | true | commitment |
| SignP256GrantSession → sign_p256_grant_session | Some(final_hash) | true | commitment |
| CreateAgentKey (create) → create_agent_key | Some(mint_label_digest) | true | commitment |
| CreateAgentKey (refresh) → create_agent_key | Some(agent_refresh_digest) | true | commitment |
| CreateP256SessionKey → create_p256_session_key | Some(mint_label_digest) | true | commitment |
| DeriveAddress → derive_address | None | false | bare nonce |
| ChangePasskey → register_passkey_ta | None | false | bare nonce |
| DeleteKey / remove_wallet → remove_wallet | None | false | bare nonce |
| RevokeAgentCredential | (DB op) | false | bare nonce |
| RevokeP256SessionKey | (DB op) | false | bare nonce |
| UnfreezeKey | host-only (no TA) | false | bare nonce |
| ExportPrivateKey (dev) → export_private_key | None | n/a | — |

> `mint_label_digest = SHA-256(tag‖wallet_id‖SHA-256(label))`,
> `agent_refresh_digest = SHA-256('AA-AGENT-REFRESH-v2'‖wallet_id‖agent_index)`.
> create vs refresh use DISTINCT tags so neither assertion can be replayed to the
> other (#115). is_refresh selects the branch; a compromised CA can't flip it (the
> client commits to the matching shape).

## The gate (run before every relevant PR)

```
python3 scripts/ca-ta-consistency.py     # exits non-zero on a same-named mismatch
```
It dumps both sides per fn and auto-flags any `TA Some + host delegate=false` for
same-named ops. Cross-crate-named ops (refresh, the sign handlers) are verified against
this matrix by hand. **Paste its output in the PR before requesting Codex review.**
