#!/usr/bin/env python3
"""CA/TA challenge-binding consistency gate.

The recurring bug class (#110, #121): the TA's `verify_passkey_for_wallet(.., payload)`
binds Some(payload) for an op, but the host's `resolve_passkey_assertion(.., delegate)`
still passes delegate=false (so the host rejects the SHA-256(nonce‖payload) commitment
as "not the bare nonce" BEFORE it reaches the TA) — or vice versa.

INVARIANT: for an op whose TA verify binds **Some(payload)**, the host MUST
**delegate=true** (so the commitment-shaped challenge reaches the TA). TA **None**
(nonce-only) pairs with host delegate=false OR true (a bare nonce passes either way),
but a host-only op (never reaches the TA) MUST be delegate=false.

This script extracts both sides per function and flags violations. RUN IT (and paste
the output) before any PR that touches a TA verify payload or a host delegate flag —
see RELEASE-CHECKLIST. Exits non-zero on a hard violation.

  python3 scripts/ca-ta-consistency.py
"""
import re
import sys
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TA = os.path.join(ROOT, "kms/ta/src/main.rs")
HOST = os.path.join(ROOT, "kms/host/src/api_server.rs")


def enclosing_fns(path, call_re):
    """Return list of (fn_name, snippet) for each call site of call_re."""
    src = open(path).read()
    lines = src.splitlines()
    fn_at = []  # (line_idx, fn_name)
    fn_re = re.compile(r"\bfn\s+([a-z_][a-z_0-9]*)\s*[(<]")
    for i, ln in enumerate(lines):
        m = fn_re.search(ln)
        if m:
            fn_at.append((i, m.group(1)))

    def fn_for(idx):
        name = "?"
        for li, n in fn_at:
            if li <= idx:
                name = n
            else:
                break
        return name

    out = []
    for m in re.finditer(call_re, src):
        idx = src[: m.start()].count("\n")
        # grab the call args up to the matching close (cheap: next ~6 lines)
        snippet = "\n".join(lines[idx : idx + 8])
        out.append((fn_for(idx), snippet))
    return out


def main():
    # TA: verify_passkey_for_wallet(.., None | Some(..))
    ta = {}
    for fn, snip in enclosing_fns(TA, r"verify_passkey_for_wallet\("):
        body = snip.split(")?")[0]
        kind = "Some" if "Some(" in body else ("None" if re.search(r",\s*None", body) else "?")
        ta.setdefault(fn, set()).add(kind)

    # HOST: resolve_passkey_assertion(.., <delegate>)
    host = {}
    for fn, snip in enclosing_fns(HOST, r"resolve_passkey_assertion\("):
        body = snip.split(".await")[0]
        # last bool-ish arg before the close paren
        if re.search(r",\s*true\s*[,)]|\btrue,\s*\n\s*\)", body) or re.search(r"true,?\s*\)", body):
            d = "true"
        elif "false" in body:
            d = "false"
        else:
            d = "VAR"
        host.setdefault(fn, set()).add(d)

    print("== TA  verify_passkey_for_wallet payload (by fn) ==")
    for fn in sorted(ta):
        print(f"  {fn:34} {'/'.join(sorted(ta[fn]))}")
    print("\n== HOST resolve_passkey_assertion delegate (by fn) ==")
    for fn in sorted(host):
        print(f"  {fn:34} {'/'.join(sorted(host[fn]))}")

    # auto-check: fns present on BOTH sides (same name) must satisfy Some<->delegate=true
    print("\n== consistency (same-named ops on both sides) ==")
    fails = 0
    for fn in sorted(set(ta) & set(host)):
        tp, hd = ta[fn], host[fn]
        ok = True
        if "Some" in tp and "true" not in hd:
            ok = False
        if tp == {"None"} and hd == {"false"}:
            ok = True  # nonce-only, host strict — fine
        # TA Some + host true => fine; TA None + host true => fine (bare nonce passes)
        mark = "OK " if ok else "MISMATCH"
        if not ok:
            fails += 1
        print(f"  [{mark}] {fn:30} TA={'/'.join(sorted(tp))}  host_delegate={'/'.join(sorted(hd))}")
    print(f"\n{'ALL CONSISTENT' if not fails else str(fails)+' MISMATCH(es) — TA binds Some(payload) but host delegate=false'}")
    print("(Ops only on one side are cross-crate-named — verify against docs/design/ca-ta-consistency-matrix.md by hand.)")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
