#!/usr/bin/env bash
# qemu/test.sh — 完整回归测试套件（针对 QEMU 环境）
#
# 测试阶段：
#   P0 健康检查        — /health, /version
#   P1 密钥生命周期    — CreateKey, Sign, GetPublicKey, DeleteKey
#   P2 WebAuthn 流程   — Register, Authenticate
#   P3 新功能回归      — SignTypedData, grant-session, P256 session key
#   P4 安全负向测试    — 无 auth 拒绝, passkey 错误拒绝
#
# 用法：
#   ./qemu/test.sh              # 全部测试
#   ./qemu/test.sh p0           # 仅健康检查
#   ./qemu/test.sh p1           # 仅密钥生命周期
#   ./qemu/test.sh regression   # P0+P1+P3 (快速回归)
#   ./qemu/test.sh security     # P4 安全负向测试

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
source "$REPO_ROOT/qemu/lib/log.sh"

BASE_URL="${KMS_BASE_URL:-http://localhost:3000}"
PASS=0
FAIL=0
SKIP=0

# ── Passkey fixture（来自 kms/test/test-fixtures/user1.json）─────────────
# CreateKey 要求 65 字节非压缩 P-256 公钥；DeriveAddress/Sign 要求 passkey 断言。
# 依赖：pip3 install cryptography（用于 p256_helper.py）
FIXTURE="$REPO_ROOT/kms/test/test-fixtures/user1.json"
P256_HELPER="$REPO_ROOT/kms/test/p256_helper.py"
TEST_PUBKEY=""
TEST_PEM=""
PASSKEY_AVAILABLE=false

if [ -f "$FIXTURE" ] && [ -f "$P256_HELPER" ]; then
    TEST_PUBKEY=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['public_key_hex'])" 2>/dev/null || echo "")
    TEST_PEM=$(python3 -c "import json; print(json.load(open('$FIXTURE'))['private_key_pem'])" 2>/dev/null || echo "")
    # 验证 p256_helper 可用
    if python3 -c "from cryptography.hazmat.primitives.asymmetric import ec" 2>/dev/null; then
        PASSKEY_AVAILABLE=true
    fi
fi
if [ -z "$TEST_PUBKEY" ]; then
    TEST_PUBKEY="0x04c2eb736467f904d93574842296e8f08c4f9d3d1b7a6ab651d2f3dae12497839f6cd34dcd5b1f2d5eaad9283f9b3368690c47dc72639b4b4ee581edab5704498d"
fi

# 生成 passkey 断言（用测试私钥签一个随机 challenge）
make_assertion() {
    python3 "$P256_HELPER" assertion "$TEST_PEM"
}

# 将 assertion JSON → Passkey 请求字段
make_passkey_json() {
    local assertion="$1"
    local auth_data cdh sig_r sig_s sig
    auth_data=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['authenticator_data'])")
    cdh=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_data_hash'])")
    sig_r=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_r'])")
    sig_s=$(echo "$assertion" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature_s'])")
    sig="${sig_r}${sig_s}"
    echo "{\"AuthenticatorData\":\"$auth_data\",\"ClientDataHash\":\"$cdh\",\"Signature\":\"$sig\"}"
}

# ── 测试工具 ──────────────────────────────────────────────────────────────
assert_http() {
    local desc="$1" expected_code="$2" url="$3"
    shift 3
    local actual_code
    actual_code=$(curl -s -o /dev/null -w "%{http_code}" "$@" "$url")
    if [ "$actual_code" = "$expected_code" ]; then
        log_info "  PASS: $desc [HTTP $actual_code]"
        ((PASS++)) || true
    else
        log_error "  FAIL: $desc [expected=$expected_code got=$actual_code]"
        ((FAIL++)) || true
    fi
}

assert_json_field() {
    local desc="$1" url="$2" field="$3" expected="$4"
    shift 4
    local actual
    actual=$(curl -s "$@" "$url" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('$field',''))" 2>/dev/null || echo "")
    if [ "$actual" = "$expected" ]; then
        log_info "  PASS: $desc [$field=$actual]"
        ((PASS++)) || true
    else
        log_error "  FAIL: $desc [$field: expected='$expected' got='$actual']"
        ((FAIL++)) || true
    fi
}

skip_test() {
    log_warn "  SKIP: $1"
    ((SKIP++)) || true
}

# ── P0: 健康检查 ──────────────────────────────────────────────────────────
test_p0_health() {
    log_step "P0: 健康检查"
    assert_http "GET /health" "200" "$BASE_URL/health"
    assert_http "GET /version" "200" "$BASE_URL/version"
    assert_json_field "version format" "$BASE_URL/version" "version" "0.19.0"
    assert_http "GET / (stats dashboard)" "200" "$BASE_URL/"
    assert_http "GET /test" "200" "$BASE_URL/test"
    assert_http "GET /QueueStatus" "200" "$BASE_URL/QueueStatus"
}

# ── P1: 密钥生命周期 ──────────────────────────────────────────────────────
test_p1_key_lifecycle() {
    log_step "P1: 密钥生命周期"

    # CreateKey（必填字段：KeyUsage, Origin, PasskeyPublicKey）
    local key_id
    key_id=$(curl -s -X POST "$BASE_URL/CreateKey" \
        -H "Content-Type: application/json" \
        -H "x-amz-target: TrentService.CreateKey" \
        -d "{\"Description\":\"qemu-test-$(date +%s)\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\",\"PasskeyPublicKey\":\"$TEST_PUBKEY\"}" \
        | python3 -c "import sys,json; print(json.load(sys.stdin).get('KeyMetadata',{}).get('KeyId',''))" 2>/dev/null || echo "")

    if [ -z "$key_id" ]; then
        log_error "  FAIL: CreateKey — no KeyId returned"
        ((FAIL++)) || true
        return
    fi
    log_info "  PASS: CreateKey [KeyId=$key_id]"
    ((PASS++)) || true

    # DescribeKey（需要 x-amz-target 头）
    assert_http "DescribeKey" "200" "$BASE_URL/DescribeKey" \
        -X POST -H "Content-Type: application/json" \
        -H "x-amz-target: TrentService.DescribeKey" \
        -d "{\"KeyId\":\"$key_id\"}"

    # GetPublicKey（需要 x-amz-target 头）
    assert_http "GetPublicKey" "200" "$BASE_URL/GetPublicKey" \
        -X POST -H "Content-Type: application/json" \
        -H "x-amz-target: TrentService.GetPublicKey" \
        -d "{\"KeyId\":\"$key_id\"}"

    # ListKeys（需要 x-amz-target 头）
    assert_http "ListKeys" "200" "$BASE_URL/ListKeys" \
        -X POST -H "Content-Type: application/json" \
        -H "x-amz-target: TrentService.ListKeys" \
        -d '{}'

    # KeyStatus — 等待地址派生完成（最多30秒）
    local key_ready=false
    for i in $(seq 1 10); do
        local status
        status=$(curl -s "$BASE_URL/KeyStatus?KeyId=$key_id" \
            | python3 -c "import sys,json; print(json.load(sys.stdin).get('Status','unknown'))" 2>/dev/null || echo "error")
        if [ "$status" = "ready" ]; then
            log_info "  PASS: KeyStatus ready [poll=$i]"
            ((PASS++)) || true
            key_ready=true
            break
        fi
        sleep 3
    done
    $key_ready || { log_error "  FAIL: KeyStatus timeout after 30s"; ((FAIL++)) || true; }

    # DeriveAddress / Sign / SignHash — 需要 passkey 断言
    if $PASSKEY_AVAILABLE; then
        local passkey_json
        passkey_json=$(make_passkey_json "$(make_assertion)")
        assert_http "DeriveAddress (m/44'/60'/0'/0/0)" "200" "$BASE_URL/DeriveAddress" \
            -X POST -H "Content-Type: application/json" \
            -H "x-amz-target: TrentService.DeriveAddress" \
            -d "{\"KeyId\":\"$key_id\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Passkey\":$passkey_json}"

        passkey_json=$(make_passkey_json "$(make_assertion)")
        assert_http "SignHash (32-byte)" "200" "$BASE_URL/SignHash" \
            -X POST -H "Content-Type: application/json" \
            -H "x-amz-target: TrentService.SignHash" \
            -d "{\"KeyId\":\"$key_id\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Hash\":\"0xa1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2\",\"Passkey\":$passkey_json}"

        passkey_json=$(make_passkey_json "$(make_assertion)")
        assert_http "Sign (message)" "200" "$BASE_URL/Sign" \
            -X POST -H "Content-Type: application/json" \
            -H "x-amz-target: TrentService.Sign" \
            -d "{\"KeyId\":\"$key_id\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Message\":\"0x48656c6c6f\",\"Passkey\":$passkey_json}"
    else
        skip_test "DeriveAddress (pip3 install cryptography 以启用 passkey 测试)"
        skip_test "SignHash (pip3 install cryptography 以启用 passkey 测试)"
        skip_test "Sign message (pip3 install cryptography 以启用 passkey 测试)"
    fi

    log_info "  [KeyId=$key_id 测试完成，保留用于后续阶段]"
    # 导出 key_id 给后续测试
    export TEST_KEY_ID="$key_id"
}

# ── P2: WebAuthn 流程（需要硬件 authenticator，CI 跳过）─────────────────
test_p2_webauthn() {
    log_step "P2: WebAuthn 流程"
    if [ "${CI:-}" = "true" ] || [ "${SKIP_WEBAUTHN:-}" = "1" ]; then
        skip_test "WebAuthn 需要交互式 authenticator，CI 跳过"
        return
    fi

    # BeginRegistration
    assert_http "BeginRegistration" "200" "$BASE_URL/BeginRegistration" \
        -X POST -H "Content-Type: application/json" \
        -d '{"username":"qemu-test-user","display_name":"QEMU Test"}'

    skip_test "CompleteRegistration / BeginAuthentication 需要真实 FIDO2 设备"
}

# ── P3: v0.19.0 新功能回归 ────────────────────────────────────────────────
test_p3_new_features() {
    log_step "P3: v0.19.0 新功能回归"

    # SignTypedData — 无 auth 应返回 400（业务拒绝，非 HTTP-level 401）
    assert_http "kms/SignTypedData (no auth) → 400" "400" "$BASE_URL/kms/SignTypedData" \
        -X POST -H "Content-Type: application/json" \
        -d '{"keyId":"test","domain":{},"types":{},"primaryType":"","message":{}}'

    # sign-grant-session — 无 auth 应拒绝（400）
    assert_http "kms/sign-grant-session (no auth) → 400" "400" "$BASE_URL/kms/sign-grant-session" \
        -X POST -H "Content-Type: application/json" \
        -d '{"key_id":"test","chain_id":1,"validator_address":"0x0000000000000000000000000000000000000001","account":"0x0000000000000000000000000000000000000002","session_key":"0x0000000000000000000000000000000000000003","expiry":9999999999,"contract_scope":"0x00000000","selector_scope":"0x00000000","velocity_limit":0,"velocity_window":0,"call_targets":[],"selector_allowlist":[],"nonce":0}'

    # sign-p256-grant-session — 无 auth 应拒绝（400）
    assert_http "kms/sign-p256-grant-session (no auth) → 400" "400" "$BASE_URL/kms/sign-p256-grant-session" \
        -X POST -H "Content-Type: application/json" \
        -d '{"key_id":"test","chain_id":1,"validator_address":"0x0000000000000000000000000000000000000001","account":"0x0000000000000000000000000000000000000002","key_x":"0000000000000000000000000000000000000000000000000000000000000001","key_y":"0000000000000000000000000000000000000000000000000000000000000002","expiry":9999999999,"contract_scope":"0x00000000","selector_scope":"0x00000000","velocity_limit":0,"velocity_window":0,"call_targets":[],"selector_allowlist":[],"nonce":0}'

    # create-p256-session-key — 无 auth 应拒绝（400）
    assert_http "kms/create-p256-session-key (no auth) → 400" "400" "$BASE_URL/kms/create-p256-session-key" \
        -X POST -H "Content-Type: application/json" \
        -d '{"key_id":"test"}'

    # JwtHmacSign 已删除（Issue #16）— 不应返回 200
    local jwt_code
    jwt_code=$(/usr/bin/curl -s -o /dev/null -w "%{http_code}" -X POST "$BASE_URL/JwtHmacSign" \
        -H "Content-Type: application/json" -d '{}')
    if [ "$jwt_code" != "200" ]; then
        log_info "  PASS: JwtHmacSign removed → HTTP $jwt_code (not 200)"
        ((PASS++)) || true
    else
        log_error "  FAIL: JwtHmacSign still returns 200 — should be removed"
        ((FAIL++)) || true
    fi

    log_info "  新功能路由均已注册，auth 拒绝行为正确"
}

# ── P4: 安全负向测试 ──────────────────────────────────────────────────────
test_p4_security() {
    log_step "P4: 安全负向测试"

    # 不存在的 KeyId（服务器返回 400 业务错误，非 404）
    assert_http "Sign (nonexistent key) → 400" "400" "$BASE_URL/Sign" \
        -X POST -H "Content-Type: application/json" \
        -H "x-amz-target: TrentService.Sign" \
        -d '{"KeyId":"nonexistent-key-00000000-0000-0000-0000-000000000000","Message":"68656c6c6f","MessageType":"RAW"}'

    # 格式错误的请求（有 x-amz-target 头，body 错误 → 400）
    assert_http "Sign (malformed JSON) → 400" "400" "$BASE_URL/Sign" \
        -X POST -H "Content-Type: application/json" \
        -H "x-amz-target: TrentService.Sign" \
        -d 'this is not json'

    # 无 x-amz-target 头 → 500（warp rejection）
    assert_http "Sign (missing header) → 500" "500" "$BASE_URL/Sign" \
        -X POST -H "Content-Type: application/json" \
        -d '{"KeyId":"test","Message":"00","MessageType":"RAW"}'

    # 正确路径存在（不返回 404）
    assert_http "GET /version → 200 (not 404)" "200" "$BASE_URL/version"
}

# ── 汇总 ──────────────────────────────────────────────────────────────────
print_summary() {
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  测试结果汇总"
    echo "  PASS: $PASS  FAIL: $FAIL  SKIP: $SKIP"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    if [ "$FAIL" -gt 0 ]; then
        log_error "❌ 有 $FAIL 个测试失败"
        exit 1
    else
        log_info "✅ 全部通过（跳过 $SKIP 个）"
    fi
}

# ── 先验证 KMS 是否可达 ──────────────────────────────────────────────────
preflight() {
    log_step "预检：连接 $BASE_URL"
    if ! curl -sf "$BASE_URL/health" &>/dev/null; then
        log_error "KMS 不可达：$BASE_URL/health"
        log_error "请先执行: ./qemu/deploy.sh"
        exit 1
    fi
    log_info "KMS 在线 ✓"
}

# ── main ─────────────────────────────────────────────────────────────────
preflight

case "${1:-all}" in
    p0)         test_p0_health ;;
    p1)         test_p1_key_lifecycle ;;
    p2)         test_p2_webauthn ;;
    p3)         test_p3_new_features ;;
    p4|security) test_p4_security ;;
    regression)
        test_p0_health
        test_p1_key_lifecycle
        test_p3_new_features
        test_p4_security
        ;;
    all)
        test_p0_health
        test_p1_key_lifecycle
        test_p2_webauthn
        test_p3_new_features
        test_p4_security
        ;;
    *)
        echo "用法: $0 [p0|p1|p2|p3|p4|regression|security|all]"
        exit 1 ;;
esac

print_summary
