#!/usr/bin/env bash
# test-release-sign.sh —— release-sign.sh(发版签发工具)测试。
# 重点:①signer 产物必须能过节点 aastar-node-updater.sh 的 load_manifest(防两份手写 schema 漂移);
#       ②--dry-run 绝不触碰 $OUT + 有效调用真能 rc=0(区分「拒绝」与「崩溃」);③多行/超长 notes、
#       越目录 channel 被 signer 拒(断言**具体错误信息**,不只 rc);④**读回基线必须验签**
#       (未签名/网络失败一律 fail-closed,不把未验证输入洗进签名产物);⑤metadata_version 单调。
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
SIGN="$(cd "$HERE/../../deploy/updater" && pwd)/release-sign.sh"
UPDATER="$(cd "$HERE/../../deploy/updater" && pwd)/aastar-node-updater.sh"

command -v minisign >/dev/null || { echo "SKIP: 需 minisign"; exit 0; }
command -v jq >/dev/null || { echo "SKIP: 需 jq"; exit 0; }
command -v curl >/dev/null || { echo "SKIP: 需 curl(读回基线测试)"; exit 0; }

PASS=0; FAIL=0
ok()  { echo "  PASS $*"; PASS=$((PASS+1)); }
bad() { echo "  FAIL $*"; FAIL=$((FAIL+1)); }

ROOT="$(mktemp -d)" || exit 1
trap 'rm -rf "$ROOT"' EXIT
minisign -G -p "$ROOT/pub.key" -s "$ROOT/sec.key" -W >/dev/null 2>&1 \
  || printf '\n\n' | minisign -G -p "$ROOT/pub.key" -s "$ROOT/sec.key" >/dev/null 2>&1
minisign -G -p "$ROOT/evil.pub" -s "$ROOT/evil.key" -W >/dev/null 2>&1 \
  || printf '\n\n' | minisign -G -p "$ROOT/evil.pub" -s "$ROOT/evil.key" >/dev/null 2>&1
echo "dummy node bundle" > "$ROOT/airaccount-node-v0.30.0.tar.gz"
echo "bundle31"          > "$ROOT/airaccount-node-v0.31.0.tar.gz"
OUT="$ROOT/stable.json"

# 不硬编码 --no-baseline —— 各用例按需显式传,才能真正覆盖读回路径。
sign() { bash "$SIGN" --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" "$@"; }

echo "== T1 signer 真实产物必须过节点 load_manifest(核心防漂移)=="
if sign --version 0.30.0 --severity high --security --notes "安全补丁" --no-baseline >/dev/null 2>&1 \
   && [ -f "$OUT" ] && [ -f "$OUT.minisig" ]; then
  st="$ROOT/state"; mkdir -p "$st"
  if AU_MANIFEST_BASE="file://$ROOT" AU_PUBKEY="$ROOT/pub.key" AU_STATE_DIR="$st" \
     AU_ROOT="$ROOT/opt" AU_NOTIFY_CMD=true AU_LOCK_NO_FLOCK=1 AU_TEST_MODE=1 \
     bash "$UPDATER" check >/dev/null 2>&1; then
    ok "signer 产物通过节点 load_manifest(验签+schema+新鲜度+防回滚)"
  else bad "signer 产物**未过**节点 load_manifest —— 两份 schema 已漂移!"; fi
else bad "signer 未能产出 stable.json / .minisig"; fi

echo "== T2 --dry-run 不触碰 \$OUT + 有效调用 rc=0 且 stdout 干净 JSON(区分拒绝 vs 崩溃)=="
rm -f "$OUT" "$OUT.minisig"
dj="$(sign --version 0.31.0 --no-baseline --dry-run --notes ok 2>/dev/null)"; rc=$?
{ [ "$rc" = 0 ] && printf '%s' "$dj" | jq -e '.metadata_version==1' >/dev/null; } \
  && ok "有效 dry-run rc=0 且 stdout 合法 JSON(正控制,证明拒绝≠崩溃)" || bad "有效 dry-run 未 rc=0/非 JSON(rc=$rc)"
{ [ ! -f "$OUT" ] && [ ! -f "$OUT.minisig" ]; } && ok "dry-run 未写 \$OUT/.minisig" || bad "dry-run 竟落盘"

echo "== T3 多行/超长 notes 被拒且报**具体**错误(不只 rc)=="
sign --version 0.31.0 --no-baseline --dry-run --notes $'a\nb' 2>"$ROOT/e3" >/dev/null && bad "多行 notes 未拒" || \
  { grep -q "控制字符" "$ROOT/e3" && ok "多行 notes 被拒且报『控制字符』" || bad "多行 notes 错误信息不对"; }
big="$(printf 'x%.0s' $(seq 1 300))"
sign --version 0.31.0 --no-baseline --dry-run --notes "$big" 2>"$ROOT/e3b" >/dev/null && bad "超长 notes 未拒" || \
  { grep -q "超 280" "$ROOT/e3b" && ok "超长 notes 被拒且报『超 280』" || bad "超长 notes 错误信息不对"; }

echo "== T4 channel 越目录被拒,报『channel 名非法』=="
bash "$SIGN" --version 0.30.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" \
  --channel "../../pwn" --no-baseline --dry-run 2>"$ROOT/e4" >/dev/null && bad "越目录 channel 未拒" || \
  { grep -q "channel 名非法" "$ROOT/e4" && ok "越目录 channel 被拒且报『channel 名非法』" || bad "越目录错误信息不对"; }

echo "== T5 读回基线**必须验签** —— 未签名/错误密钥的已发布 manifest 一律拒(防洗白)=="
PUB_BASE="$ROOT/pubbase"; mkdir -p "$PUB_BASE"
# 攻击者投放:metadata_version 天文数字,但**没有 .minisig**
jq -n '{metadata_version:999999999,generated_at:"2026-01-01T00:00:00Z",expires:"2030-01-01T00:00:00Z",channel:"stable",rollback_floor:"0.28.0",releases:[]}' > "$PUB_BASE/stable.json"
bash "$SIGN" --version 0.32.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" 2>"$ROOT/e5" >/dev/null \
  && bad "未签名基线竟被采信(洗白漏洞!)" || \
  { grep -qE "\.minisig|验签失败" "$ROOT/e5" && ok "未签名基线被拒(不把未验证输入洗进签名产物)" || bad "未签名基线信息不对"; }
minisign -S -s "$ROOT/evil.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" -c evil >/dev/null 2>&1
bash "$SIGN" --version 0.32.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" 2>"$ROOT/e5b" >/dev/null \
  && bad "错误密钥签名的基线竟被采信" || \
  { grep -q "验签失败" "$ROOT/e5b" && ok "错误密钥基线被拒(验签失败)" || bad "错误密钥基线信息不对"; }

echo "== T6 读回**已验签**基线 → counter/releases 从验签副本继承(不丢旧 release)=="
rm -f "$OUT" "$OUT.minisig"; rm -f "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
sign --version 0.30.0 --no-baseline --notes v1 >/dev/null 2>&1
cp "$OUT" "$PUB_BASE/stable.json"; cp "$OUT.minisig" "$PUB_BASE/stable.json.minisig"
pub_meta="$(jq -r .metadata_version "$PUB_BASE/stable.json")"
rm -f "$OUT" "$OUT.minisig"   # 新机器场景:本地 $OUT 没了,只能靠读回基线
bash "$SIGN" --version 0.31.0 --tarball "$ROOT/airaccount-node-v0.31.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" --notes v2 >/dev/null 2>&1
newmeta="$(jq -r .metadata_version "$OUT" 2>/dev/null || echo 0)"
vers="$(jq -r '[.releases[].version]|sort|join(",")' "$OUT" 2>/dev/null || echo '')"
[ "$newmeta" = "$((pub_meta+1))" ] && ok "counter 从验签基线 +1(${pub_meta}→${newmeta})" || bad "counter=$newmeta(应 $((pub_meta+1)))"
[ "$vers" = "0.30.0,0.31.0" ] && ok "releases 从验签基线继承(未丢 0.30.0):$vers" || bad "releases 丢失:$vers"

echo "== T7 读回失败(端点不可达)且未 --no-baseline → fail-closed(不静默回退成计数倒退)=="
rm -f "$OUT" "$OUT.minisig"
bash "$SIGN" --version 0.33.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$ROOT/does-not-exist" 2>"$ROOT/e7" >/dev/null \
  && bad "读回失败竟静默成功(计数可能倒退→砖化)" || \
  { grep -qE "读回已发布 manifest 失败|--no-baseline" "$ROOT/e7" && ok "读回失败 fail-closed,提示 --no-baseline" || bad "读回失败信息不对"; }

echo
echo "结果: PASS=$PASS FAIL=$FAIL"
[ "$FAIL" = 0 ]
