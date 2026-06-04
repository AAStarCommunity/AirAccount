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

# 1. Production TA must not ship with export-secrets.
#    The feature flag is the compile-time gate; verify no production build script passes it.
#    Exclude this script itself (its error-message strings would match) and filter comment lines.
if grep -rE "\-\-features[[:space:]]+export-secrets" \
        --exclude="security-check.sh" \
        scripts/ .github/ kms/scripts/ qemu/ 2>/dev/null \
        | grep -vE ':[[:space:]]*#' | grep -q .; then
    check "deploy scripts do not enable export-secrets" \
          "fail: found active --features export-secrets in deploy/CI scripts"
else
    check "deploy scripts do not enable export-secrets" "ok"
fi

# 2. Confirm the production stub for export_private_key contains the disabled error.
#    Use grep -A context to verify the cfg guard AND disabled string appear within
#    the same function body (not just anywhere in the file).
if grep -A5 '#\[cfg(not(feature = "export-secrets"))\]' kms/ta/src/main.rs \
        | grep -q 'ExportPrivateKey is disabled'; then
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
if grep -rE "(private_key\s*=\s*['\"]0x[0-9a-fA-F]{64}|mnemonic\s*=\s*['\"][a-z]+ [a-z]+ [a-z]+)" \
        src/ kms/ 2>/dev/null | grep -v "test\|example\|doc\|spec"; then
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
