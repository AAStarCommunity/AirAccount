#!/bin/bash
# Custom security checks for AirAccount KMS.
# Called by .github/workflows/security-audit.yml — exit non-zero to block merge.

set -euo pipefail

PASS=0
FAIL=0

check() {
    local label="$1"
    local result="$2"  # "ok" or "fail:<msg>"
    if [[ "$result" == "ok" ]]; then
        echo "✅  $label"
        ((PASS++)) || true
    else
        echo "❌  $label — ${result#fail:}"
        ((FAIL++)) || true
    fi
}

echo "=== AirAccount KMS security checks ==="
echo ""

# 1. Production TA must not ship with export-secrets enabled.
#    Covers all Cargo forms (cargo build --help: -F, --features <FEATURES>):
#      --features export-secrets  --features=export-secrets
#      --features foo,export-secrets  --features "export-secrets"
#      -F export-secrets  -F=export-secrets  (short alias)
#      --all-features
#    Whitelist: cargo geiger --all-features in security-audit.yml (static analysis only).
_check1=$(grep -rE "((--features|-F)[[:space:]+=].*export-secrets|--all-features)" \
        --exclude="security-check.sh" \
        scripts/ .github/ kms/scripts/ qemu/ docker/ 2>/dev/null \
        | grep -vE ':[[:space:]]*#' \
        | grep -v "security-audit.yml:.*cargo geiger.*--all-features" || true)
if [[ -n "$_check1" ]]; then
    check "deploy scripts do not enable export-secrets" \
          "fail: found --features/-F ...export-secrets or --all-features in deploy/CI scripts"
else
    check "deploy scripts do not enable export-secrets" "ok"
fi

# 1b. Decentralized KMS: production release must ship NO admin surface.
#     The /admin/purge-key endpoint is gated behind the compile-time "admin-purge"
#     feature; release deploy/CI scripts must never enable it (--all-features already
#     covered by check 1). Comment lines (e.g. the mx93-build.sh hint) are excluded.
_check1b=$(grep -rE "(--features|-F)[[:space:]+=].*admin-purge" \
        --exclude="security-check.sh" \
        scripts/ .github/ kms/scripts/ qemu/ docker/ 2>/dev/null \
        | grep -vE ':[[:space:]]*#' || true)
if [[ -n "$_check1b" ]]; then
    check "deploy scripts do not enable admin-purge" \
          "fail: found --features/-F ...admin-purge in deploy/CI scripts (no admin in release)"
else
    check "deploy scripts do not enable admin-purge" "ok"
fi

# 2. Confirm the cfg(not(export-secrets)) stub for export_private_key contains the disabled error.
#    Use awk for function-scope matching: track the cfg line → fn export_private_key → closing brace,
#    verify "ExportPrivateKey is disabled" appears inside that specific function body.
#    This is robust against future cfg(not(export-secrets)) blocks elsewhere in the file.
if awk '
  /^#\[cfg\(not\(feature = "export-secrets"\)\)\]/ { in_cfg=1; next }
  in_cfg && /fn export_private_key/ { in_stub=1; depth=0; in_cfg=0; next }
  in_cfg { in_cfg=0 }
  in_stub && /\{/ { depth++ }
  in_stub && /\}/ { depth--; if (depth==0) { in_stub=0 } }
  in_stub && /ExportPrivateKey is disabled/ { found=1 }
  END { exit !found }
' kms/ta/src/main.rs; then
    check "TA production stub rejects ExportPrivateKey" "ok"
else
    check "TA production stub rejects ExportPrivateKey" \
          "fail: missing cfg(not(export-secrets)) stub with disabled error in kms/ta/src/main.rs"
fi

# 3. export_key binary must not be copied by production deploy scripts.
if grep -rE "cp[[:space:]].*export_key" \
        --exclude="security-check.sh" \
        scripts/ kms/scripts/ qemu/ 2>/dev/null \
        | grep -vE ':[[:space:]]*#' | grep -q .; then
    check "deploy scripts do not copy export_key" \
          "fail: found active cp export_key in deploy script"
else
    check "deploy scripts do not copy export_key" "ok"
fi

# 4. No hardcoded private key material or mnemonic patterns in source.
#    Exclude test/example/doc directories by path (not line content) to avoid false positives.
if grep -rE "(private_key\s*=\s*['\"]0x[0-9a-fA-F]{64}|mnemonic\s*=\s*['\"][a-z]+ [a-z]+ [a-z]+)" \
        --exclude-dir="tests" \
        --exclude-dir="test" \
        --exclude-dir="examples" \
        --exclude-dir="docs" \
        --exclude="*_test.rs" \
        --exclude="*_spec.rs" \
        src/ kms/ 2>/dev/null | grep -q .; then
    check "no hardcoded key material in source" \
          "fail: possible hardcoded private key or mnemonic found"
else
    check "no hardcoded key material in source" "ok"
fi

echo ""
echo "=== Results: ${PASS} passed, ${FAIL} failed ==="

if [[ $FAIL -gt 0 ]]; then
    exit 1
fi
