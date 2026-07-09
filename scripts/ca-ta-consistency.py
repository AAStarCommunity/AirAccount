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

This gate now covers three previously-manual blind spots (#122):
  1. `resolve_passkey_assertion_strict(..)` call sites (the whole sign/create_key/
     derive/remove/unfreeze family routes through _strict — the old gate parsed only
     the non-strict fn and never saw them).
  2. Cross-crate-named ops via OP_MAP (e.g. host `refresh_agent_credential` re-mints
     through TA `create_agent_key`; host `sign` reaches TA `sign_transaction`).
  3. Host-authoritative ops (revoke_*, contact-binding) MUST stay delegate=false —
     they have NO TA payload binding, so a stray `true` would ship a commitment the TA
     never checks. Asserted in HOST_AUTHORITATIVE_FALSE.

RUN IT (and paste the output) before any PR that touches a TA verify payload or a host
delegate flag — see RELEASE-CHECKLIST. Exits non-zero on a hard violation.

  python3 scripts/ca-ta-consistency.py
"""
import re
import sys
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TA = os.path.join(ROOT, "kms/ta/src/main.rs")
HOST = os.path.join(ROOT, "kms/host/src/api_server.rs")

# --- audited mapping tables (#122) --------------------------------------------
# Cross-named / _strict-routed ops: host handler fn -> TA verify fn it reaches.
# Audited 2026-07-09 against kms/host/src/api_server.rs; line refs are the host
# resolve_passkey_assertion(_strict) call site. Keep in sync with
# docs/design/ca-ta-consistency-matrix.md.
OP_MAP = {
    "sign":                     "sign_transaction",    # @1991 _strict true; TA Some (also sign_message)
    "sign_hash":                "sign_hash",           # @2184 _strict true; TA Some
    "refresh_agent_credential": "create_agent_key",    # @3537 non-strict true; TA Some (is_refresh branch)
    "change_passkey":           "register_passkey_ta", # @1504 _strict false; TA None (nonce-only)
}

# Host-authoritative ops: no TA payload binding, so delegate MUST be false. A `true`
# here would send a commitment-shaped challenge the TA never verifies (security
# regression). Audited 2026-07-09.
HOST_AUTHORITATIVE_FALSE = {
    "revoke_agent_credential",   # @3611
    "revoke_p256_session_key",   # @3899
    "begin_contact_binding",     # @4000  owner-gate ceremony only
    "confirm_contact_binding",   # @4077
    "unbind_contact",            # @4119
    "unfreeze_key",              # @2337  freeze/unfreeze ceremony, no TA payload binding
}

# The resolver definitions themselves — never treat their bodies as call sites.
# NOTE (grant ops): sign_grant_session / sign_p256_grant_session do NOT take a per-op
# delegate flag — they call `resolve_grant_passkey_assertion`, which has no delegate
# argument and structurally always forwards to the TA (the TA re-binds Some(final_hash)
# at sign time). There is no host bool to misconfigure, so those TA-Some ops are
# correctly surfaced as WARN (audit-by-hand), not auto-checked here. The `true` inside
# resolve_grant_passkey_assertion is verify_authentication_response's user-verification
# flag, unrelated to challenge-value delegation.
RESOLVER_DEFS = {"resolve_passkey_assertion", "resolve_passkey_assertion_strict"}


def _fn_index(lines):
    fn_re = re.compile(r"\bfn\s+([a-z_][a-z_0-9]*)\s*[(<]")
    fn_at = []
    for i, ln in enumerate(lines):
        m = fn_re.search(ln)
        if m:
            fn_at.append((i, m.group(1)))
    return fn_at


def _fn_for(fn_at, idx):
    name = "?"
    for li, n in fn_at:
        if li <= idx:
            name = n
        else:
            break
    return name


def strip_comments(src):
    """Blank out // and /* */ comments AND the *contents* of string literals, preserving
    every newline and the total length (so line/offset math stays valid). We never parse
    inside comments or strings, so blanking their contents removes any call/comment
    pattern hiding there:
      - a commented-out fake call site `// resolve_passkey_assertion(.., true)` next to a
        real `.., false)` no longer injects a phantom delegate (Codex #1/#2/#4);
      - a string literal that happens to contain `resolve_passkey_assertion(` no longer
        registers a phantom call site (Codex Low: false alarm).
    Correct lexing of the surrounding tokens is required so the blanking itself cannot
    misfire:
      - char literals `'"'` / `b'"'` are copied through, so their embedded quote does NOT
        open a string state (which would blank real code after it — a new false negative);
      - raw strings `r"..."`, `r#"..."#`, `br#"..."#` are handled by hash-count so an
        interior `"` does not close them early (Codex Low: raw-string boundary)."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        # line comment
        if c == "/" and nxt == "/":
            out.append("  "); i += 2
            while i < n and src[i] != "\n":
                out.append("\t" if src[i] == "\t" else " "); i += 1
            continue
        # block comment
        if c == "/" and nxt == "*":
            out.append("  "); i += 2
            while i < n and not (src[i] == "*" and i + 1 < n and src[i + 1] == "/"):
                out.append(src[i] if src[i] == "\n" else " "); i += 1
            if i < n:
                out.append("  "); i += 2
            continue
        # raw string: (b)r#*"  ...  "#*   — r/br not preceded by an identifier char
        if c == "r" or (c == "b" and nxt == "r"):
            k = i + (1 if c == "r" else 2)
            h = 0
            while k < n and src[k] == "#":
                h += 1; k += 1
            prev = src[i - 1] if i > 0 else ""
            if k < n and src[k] == '"' and not (prev.isalnum() or prev == "_"):
                out.append(src[i : k + 1])  # keep (b)r#*" delimiter
                i = k + 1
                close = '"' + "#" * h
                while i < n and src[i : i + len(close)] != close:
                    out.append("\n" if src[i] == "\n" else " "); i += 1
                if i < n:
                    out.append(close); i += len(close)
                continue
        # char literal or lifetime. Copy char literals through verbatim so an embedded
        # quote (b'"') can't open a string; lifetimes ('a) have no close and fall through.
        if c == "'":
            if nxt == "\\":  # '\n' '\'' '\u{7f}' — escaped, scan to closing '
                j = i + 2
                while j < n and src[j] != "'":
                    j += 1
                out.append(src[i : j + 1]); i = j + 1; continue
            if i + 2 < n and src[i + 2] == "'":  # 'X' single char
                out.append(src[i : i + 3]); i += 3; continue
            out.append(c); i += 1; continue  # lifetime / lone '
        # normal or byte string: blank the interior
        if c == '"':
            out.append('"'); i += 1
            while i < n and src[i] != '"':
                if src[i] == "\\" and i + 1 < n:
                    out.append("  "); i += 2; continue
                out.append("\n" if src[i] == "\n" else " "); i += 1
            if i < n:
                out.append('"'); i += 1
            continue
        out.append(c); i += 1
    return "".join(out)


def _args_of_call(src, name_end):
    """Given the index just before the '(' of a call in comment-stripped src, return
    the top-level arg string, balancing nested parens. Fails safe (returns "") on an
    unbalanced call, but emits a stderr diagnostic so a truncated/malformed call site
    is not silently classified as `?`/VAR (Codex Low: no diagnostic)."""
    i = src.index("(", name_end)
    depth = 0
    for j in range(i, len(src)):
        c = src[j]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return src[i + 1 : j]
    print(f"  [warn] unbalanced '(' at line ~{src[:i].count(chr(10)) + 1} — "
          f"arg extraction incomplete, op left unclassified", file=sys.stderr)
    return ""


def _split_top_args(argstr):
    # Only (){}[ ] change nesting. `<`/`>` are NOT treated as brackets: Rust uses them
    # for comparison as well as generics, and mis-balancing them dropped the real last
    # arg to `?` (Codex #1). The call args we parse never contain angle-bracket groups.
    out, depth, cur = [], 0, ""
    for c in argstr:
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        if c == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += c
    if cur.strip():
        out.append(cur)
    return [a.strip() for a in out]


def ta_bindings():
    """TA: fn -> {Some|None} from verify_passkey_for_wallet(.., None|Some(..))."""
    src = strip_comments(open(TA).read())
    fn_at = _fn_index(src.splitlines())
    ta = {}
    for m in re.finditer(r"verify_passkey_for_wallet\s*\(", src):
        fn = _fn_for(fn_at, src[: m.start()].count("\n"))
        if fn in RESOLVER_DEFS or fn == "verify_passkey_for_wallet":
            continue
        args = _split_top_args(_args_of_call(src, m.end() - 1))
        last = args[-1] if args else ""
        kind = "Some" if last.startswith("Some") else ("None" if last == "None" else "?")
        ta.setdefault(fn, set()).add(kind)
    return ta


def host_delegates():
    """HOST: fn -> {true|false|VAR} from BOTH resolve_passkey_assertion(..) and
    resolve_passkey_assertion_strict(..). Definition bodies are excluded."""
    src = strip_comments(open(HOST).read())
    fn_at = _fn_index(src.splitlines())
    host = {}
    for m in re.finditer(r"resolve_passkey_assertion(?:_strict)?\s*\(", src):
        fn = _fn_for(fn_at, src[: m.start()].count("\n"))
        if fn in RESOLVER_DEFS:
            continue
        args = _split_top_args(_args_of_call(src, m.end() - 1))
        last = args[-1] if args else ""
        if last == "true":
            d = "true"
        elif last == "false":
            d = "false"
        else:
            d = "VAR"
        host.setdefault(fn, set()).add(d)
    return host


def _delegate_ok(tp, hd):
    """Invariant. TA Some(payload) (or unclassifiable `?`, treated conservatively as
    Some) => EVERY host delegate on that op must be exactly true: no `false` (a false
    branch masked by a sibling true is a real mismatch — Codex #3), and no `VAR`
    (unverifiable through a variable — fail loud rather than pass silent). TA None
    (nonce-only) => any host delegate is acceptable (a bare nonce passes either way)."""
    if "Some" in tp or "?" in tp:
        return hd == {"true"}
    return True


def main():
    ta = ta_bindings()
    host = host_delegates()

    print("== TA  verify_passkey_for_wallet payload (by fn) ==")
    for fn in sorted(ta):
        print(f"  {fn:34} {'/'.join(sorted(ta[fn]))}")
    print("\n== HOST resolve_passkey_assertion[_strict] delegate (by fn) ==")
    for fn in sorted(host):
        print(f"  {fn:34} {'/'.join(sorted(host[fn]))}")

    fails = []

    # (1) same-named ops on both sides
    print("\n== consistency · same-named ops ==")
    for fn in sorted(set(ta) & set(host)):
        tp, hd = ta[fn], host[fn]
        ok = _delegate_ok(tp, hd)
        print(f"  [{'OK ' if ok else 'MISMATCH'}] {fn:28} TA={'/'.join(sorted(tp))}  host={'/'.join(sorted(hd))}")
        if not ok:
            fails.append(f"same-name {fn}: TA {'/'.join(sorted(tp))} requires host delegate exactly true, got {'/'.join(sorted(hd))}")

    # (2) cross-named / _strict-routed ops via OP_MAP
    print("\n== consistency · cross-named (OP_MAP) ==")
    for hfn, tfn in sorted(OP_MAP.items()):
        if hfn not in host:
            fails.append(f"OP_MAP host fn '{hfn}' not found — mapping stale?")
            print(f"  [STALE   ] host '{hfn}' absent (mapping stale?)")
            continue
        if tfn not in ta:
            fails.append(f"OP_MAP TA fn '{tfn}' not found — mapping stale?")
            print(f"  [STALE   ] TA '{tfn}' absent (mapping stale?)")
            continue
        tp, hd = ta[tfn], host[hfn]
        ok = _delegate_ok(tp, hd)
        print(f"  [{'OK ' if ok else 'MISMATCH'}] {hfn:28} -> TA {tfn:22} TA={'/'.join(sorted(tp))}  host={'/'.join(sorted(hd))}")
        if not ok:
            fails.append(f"OP_MAP {hfn}->{tfn}: TA {'/'.join(sorted(tp))} requires host delegate exactly true, got {'/'.join(sorted(hd))}")

    # (3) host-authoritative ops must be strictly delegate=false
    print("\n== consistency · host-authoritative (must be delegate=false) ==")
    for fn in sorted(HOST_AUTHORITATIVE_FALSE):
        if fn not in host:
            fails.append(f"host-authoritative fn '{fn}' not found — set stale?")
            print(f"  [STALE   ] {fn} absent (set stale?)")
            continue
        hd = host[fn]
        ok = hd == {"false"}
        print(f"  [{'OK ' if ok else 'MISMATCH'}] {fn:28} host={'/'.join(sorted(hd))} (expect false)")
        if not ok:
            fails.append(f"host-authoritative {fn}: expected delegate=false, got {'/'.join(sorted(hd))}")

    # (4) WARN: TA ops that bind Some (or unclassified) but are covered by NEITHER a
    # same-name check NOR OP_MAP. These route to the host through a shared path (e.g.
    # sign_grant_session via the core sign() handler) and can't be auto-verified — they
    # are NOT a failure, but must be audited by hand against the matrix so a real
    # delegate=false regression on one of them cannot slip through silently.
    covered_ta = (set(ta) & set(host)) | set(OP_MAP.values())
    uncovered = sorted(fn for fn in ta if ("Some" in ta[fn] or "?" in ta[fn]) and fn not in covered_ta)
    print("\n== coverage · TA Some/? ops not auto-checked (audit by hand) ==")
    if uncovered:
        for fn in uncovered:
            print(f"  [WARN] {fn:28} TA={'/'.join(sorted(ta[fn]))} — no same-name host fn / not in OP_MAP")
        print("  → verify each against docs/design/ca-ta-consistency-matrix.md (host path delegates correctly).")
    else:
        print("  (none — every TA Some/? op is auto-checked)")

    print()
    if fails:
        print(f"{len(fails)} MISMATCH(es):")
        for f in fails:
            print(f"  - {f}")
    else:
        print("ALL CONSISTENT")
    print("(Ops with a host delegate=VAR are resolved through a variable — audit the "
          "call site by hand against docs/design/ca-ta-consistency-matrix.md.)")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
