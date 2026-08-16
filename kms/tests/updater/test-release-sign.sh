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

echo "== T8 读回基线**已过期** → 拒绝(finding1a:验签≠新鲜,防重放旧签名撤销撤销)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
jq -n '{metadata_version:5,generated_at:"2020-01-01T00:00:00Z",expires:"2020-01-08T00:00:00Z",channel:"stable",rollback_floor:"0.31.0",releases:[{version:"0.31.0",security:true,severity:"high",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:"s",canary_ring:[]}]}' > "$PUB_BASE/stable.json"
minisign -S -s "$ROOT/sec.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" >/dev/null 2>&1
bash "$SIGN" --version 0.32.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" 2>"$ROOT/e8" >/dev/null \
  && bad "过期基线竟被采信(重放撤销撤销)" || \
  { grep -q "已过期" "$ROOT/e8" && ok "过期基线被拒(报『已过期』)" || bad "过期基线错误信息不对: $(cat "$ROOT/e8")"; }

echo "== T9 读回基线 channel 不符(beta 放到 stable URL)→ 拒绝(finding2:签名保真不保来源)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
jq -n '{metadata_version:5,generated_at:"2035-01-01T00:00:00Z",expires:"2035-01-08T00:00:00Z",channel:"beta",rollback_floor:"0.20.0",releases:[{version:"0.40.0",security:false,severity:"none",notes:"beta",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:"s",canary_ring:[]}]}' > "$PUB_BASE/stable.json"
minisign -S -s "$ROOT/sec.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" >/dev/null 2>&1
bash "$SIGN" --version 0.32.0 --channel stable --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" 2>"$ROOT/e9" >/dev/null \
  && bad "channel 不符竟被继承(beta 污染 stable)" || \
  { grep -q "channel" "$ROOT/e9" && ok "channel 不符被拒(beta≠stable)" || bad "channel 错误信息不对: $(cat "$ROOT/e9")"; }

S64=0000000000000000000000000000000000000000000000000000000000000000
echo "== T10 本地领先:releases **自由并集**(无需 flag)+ floor **单调 max**(#196 R6 墓碑模型)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
sign --version 0.30.0 --no-baseline --rollback-floor 0.31.0 --notes l1 >/dev/null 2>&1
sign --version 0.31.0 --no-baseline --notes l2 >/dev/null 2>&1   # 本地 meta=2 floor=0.31.0 releases=[0.31.0,0.30.0]
jq -n --arg s "$S64" '{metadata_version:1,generated_at:"2035-01-01T00:00:00Z",expires:"2035-01-08T00:00:00Z",channel:"stable",rollback_floor:"0.28.0",revoked:[],releases:[{version:"0.30.0",security:false,severity:"none",notes:"stale",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]}]}' > "$PUB_BASE/stable.json"
minisign -S -s "$ROOT/sec.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" >/dev/null 2>&1
bash "$SIGN" --version 0.32.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" --notes v3 >/dev/null 2>"$ROOT/e10"
nf="$(jq -r .rollback_floor "$OUT" 2>/dev/null)"; nv="$(jq -r '[.releases[].version]|sort|join(",")' "$OUT" 2>/dev/null)"
[ "$nf" = "0.31.0" ] && ok "floor 单调:陈旧基线 0.28.0 未压过 0.31.0" || bad "floor=$nf(应 0.31.0)"
[ "$nv" = "0.30.0,0.31.0,0.32.0" ] && ok "releases 自由并集(无需 flag):$nv" || bad "releases=$nv(应 0.30.0,0.31.0,0.32.0)"

echo "== T11 --revoke:撤销版本 → 从 releases 剔除 + 进 revoked[] 墓碑(#196 R6 撤销机制)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
sign --version 0.30.0 --no-baseline --notes a >/dev/null 2>&1
sign --version 0.32.0 --no-baseline --notes b >/dev/null 2>&1   # releases=[0.32.0,0.30.0]
sign --version 0.33.0 --no-baseline --revoke 0.32.0 --notes c >/dev/null 2>&1
rv="$(jq -c '.revoked' "$OUT" 2>/dev/null)"; nv="$(jq -r '[.releases[].version]|sort|join(",")' "$OUT" 2>/dev/null)"
echo "$rv" | grep -q '"0.32.0"' && ok "0.32.0 进 revoked[]($rv)" || bad "revoked=$rv"
echo "$nv" | grep -q "0.32.0" && bad "0.32.0 仍在 releases=$nv" || ok "0.32.0 从 releases 剔除($nv)"
[ "$nv" = "0.30.0,0.33.0" ] && ok "releases=基线保留(0.30.0)+新版(0.33.0),撤销版剔除" || bad "releases=$nv(应 0.30.0,0.33.0)"

echo "== T12 继承的 canary_ring **数值**元素 → schema 自检拒绝(与节点 :518 谓词对齐,防漂移)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
jq -n '{metadata_version:1,generated_at:"2035-01-01T00:00:00Z",expires:"2035-01-08T00:00:00Z",channel:"stable",rollback_floor:"0.28.0",releases:[{version:"0.30.0",security:false,severity:"none",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:"0000000000000000000000000000000000000000000000000000000000000000",canary_ring:[1,2]}]}' > "$PUB_BASE/stable.json"
minisign -S -s "$ROOT/sec.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" >/dev/null 2>&1
bash "$SIGN" --version 0.32.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" --notes v5 2>"$ROOT/e12" >/dev/null \
  && bad "数值 canary_ring 竟签发(与节点谓词漂移,会致全网 fail-closed 冻结)" || \
  { grep -q "schema" "$ROOT/e12" && ok "数值 canary_ring 被 schema 自检拒(与节点对齐)" || bad "canary 错误信息不对: $(cat "$ROOT/e12")"; }

echo "== T13 revoked **单调**:陈旧基线不能 un-revoke,且撤销版即便被列在 releases 也剔除(#196 R6)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
# 本地已撤销 0.32.0(revoked=[0.32.0]),但本地 releases 里(错误地)还留着 0.32.0
jq -n --arg s "$S64" '{metadata_version:3,rollback_floor:"0.30.0",revoked:["0.32.0"],releases:[
  {version:"0.32.0",security:false,severity:"none",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]},
  {version:"0.30.0",security:false,severity:"none",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]}]}' > "$OUT"
minisign -S -s "$ROOT/sec.key" -m "$OUT" -x "$OUT.minisig" >/dev/null 2>&1   # 签本地 $OUT(#196 R8:读 LOCAL_* 前要验签)
# 网络基线**陈旧**(meta=2 < 本地 3):revoked 空(撤销发生前),releases 含 0.32.0
jq -n --arg s "$S64" '{metadata_version:2,generated_at:"2035-01-01T00:00:00Z",expires:"2035-01-08T00:00:00Z",channel:"stable",rollback_floor:"0.30.0",revoked:[],releases:[{version:"0.32.0",security:false,severity:"none",notes:"stale",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]},{version:"0.30.0",security:false,severity:"none",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]}]}' > "$PUB_BASE/stable.json"
minisign -S -s "$ROOT/sec.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" >/dev/null 2>&1
bash "$SIGN" --version 0.34.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" --notes v7 >/dev/null 2>"$ROOT/e13"
rv3="$(jq -c '.revoked' "$OUT" 2>/dev/null)"; nv3="$(jq -r '[.releases[].version]|sort|join(",")' "$OUT" 2>/dev/null)"
echo "$rv3" | grep -q '"0.32.0"' && ok "revoked 单调:陈旧基线未 un-revoke 0.32.0($rv3)" || bad "revoked 丢了 0.32.0=$rv3"
echo "$nv3" | grep -q "0.32.0" && bad "被撤销的 0.32.0 复活(基线/本地 releases 里有就漏)=$nv3" || ok "0.32.0 被 revoked 剔除,不复活($nv3)"

echo "== T14 重签一个已撤销的版本(--version 撞 revoked)→ 拒绝(#196 R6,堵最简单那条复活路)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
jq -n --arg s "$S64" '{metadata_version:3,generated_at:"2035-01-01T00:00:00Z",expires:"2035-01-08T00:00:00Z",channel:"stable",rollback_floor:"0.30.0",revoked:["0.32.0"],releases:[{version:"0.30.0",security:false,severity:"none",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]}]}' > "$PUB_BASE/stable.json"
minisign -S -s "$ROOT/sec.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" >/dev/null 2>&1
bash "$SIGN" --version 0.32.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" --notes v8 2>"$ROOT/e14" >/dev/null \
  && bad "重签已撤销的 0.32.0 竟成功(绕过墓碑复活)" || \
  { grep -qE "revoked|撤销" "$ROOT/e14" && ok "重签已撤销版本被拒(revoked 墓碑)" || bad "错误信息不对: $(cat "$ROOT/e14")"; }

echo "== T15 纯撤销:--revoke 不带 --version(事故当下无新版本可发也能撤)(#196 R7 finding3)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
sign --version 0.30.0 --no-baseline --notes a >/dev/null 2>&1
sign --version 0.32.0 --no-baseline --notes b >/dev/null 2>&1
bash "$SIGN" --out "$OUT" --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --no-baseline --revoke 0.32.0 --notes r >/dev/null 2>&1 && prc=0 || prc=1
[ "$prc" = 0 ] && ok "纯撤销 rc=0(无 --version 也能跑)" || bad "纯撤销失败 rc=$prc"
[ "$(jq -r '[.releases[].version]|sort|join(",")' "$OUT" 2>/dev/null)" = "0.30.0" ] && ok "0.32.0 剔除,releases=[0.30.0]" || bad "releases=$(jq -c '[.releases[].version]' "$OUT")"
jq -e '.revoked|index("0.32.0")' "$OUT" >/dev/null 2>&1 && ok "0.32.0 进 revoked[]" || bad "revoked=$(jq -c .revoked "$OUT")"
[ "$(jq -r .metadata_version "$OUT" 2>/dev/null)" = 3 ] && ok "meta 递增到 3(纯撤销也 bump counter)" || bad "meta=$(jq -r .metadata_version "$OUT")"

echo "== T16 迁移安全:基线**无 revoked 字段**(墓碑前发布)→ 本地独有条目**不并入**(#196 R7 finding1)=="
rm -f "$OUT" "$OUT.minisig" "$PUB_BASE/stable.json" "$PUB_BASE/stable.json.minisig"
# 本地 $OUT:meta=2,带一条本地独有 0.31.0(可能是老式删条目撤销的,无法判定)
jq -n --arg s "$S64" '{metadata_version:2,rollback_floor:"0.30.0",revoked:[],releases:[
  {version:"0.31.0",security:false,severity:"none",notes:"local-only",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]},
  {version:"0.30.0",security:false,severity:"none",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]}]}' > "$OUT"
minisign -S -s "$ROOT/sec.key" -m "$OUT" -x "$OUT.minisig" >/dev/null 2>&1   # 签本地 $OUT(#196 R8:读 LOCAL_* 前要验签)
# 基线**无 revoked 字段**(墓碑之前),0.31.0 已用老式删条目撤销(releases 只 0.30.0)
jq -n --arg s "$S64" '{metadata_version:3,generated_at:"2035-01-01T00:00:00Z",expires:"2035-01-08T00:00:00Z",channel:"stable",rollback_floor:"0.30.0",releases:[{version:"0.30.0",security:false,severity:"none",notes:"auth",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]}]}' > "$PUB_BASE/stable.json"
minisign -S -s "$ROOT/sec.key" -m "$PUB_BASE/stable.json" -x "$PUB_BASE/stable.json.minisig" >/dev/null 2>&1
bash "$SIGN" --version 0.34.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --base-url "file://$PUB_BASE" --notes v9 2>"$ROOT/e16" >/dev/null
nv16="$(jq -r '[.releases[].version]|sort|join(",")' "$OUT" 2>/dev/null)"
echo "$nv16" | grep -q "0.31.0" && bad "本地独有 0.31.0 被静默洗入(迁移不安全)=$nv16" || ok "基线无 revoked 字段→本地独有 0.31.0 不并入($nv16)"
grep -q "无 revoked 字段" "$ROOT/e16" && ok "迁移已披露(不静默丢/不静默洗)" || bad "无迁移披露: $(cat "$ROOT/e16")"

echo "== T17 --no-baseline:revoked 仅本地、可能缩水 → 响亮告警(#196 R7 finding2)=="
rm -f "$OUT" "$OUT.minisig"
sign --version 0.30.0 --no-baseline --notes a >/dev/null 2>&1
sign --version 0.31.0 --no-baseline --notes b 2>"$ROOT/e17" >/dev/null
grep -q "no-baseline.*revoked 仅来自本地" "$ROOT/e17" && ok "--no-baseline 缩 revoked 有响亮告警" || bad "无 --no-baseline 告警: $(cat "$ROOT/e17")"

echo "== T18 本地 \$OUT 未签名/被篡改 → 读 LOCAL_* 前验签门拦下(#196 R8 finding1/2:防未签名无界驱动 counter=永久砖化)=="
rm -f "$OUT" "$OUT.minisig"
# (a) 无 .minisig 的本地 $OUT(meta=9e9)→ 默认拒
jq -n --arg s "$S64" '{metadata_version:999999999,rollback_floor:"0.28.0",revoked:[],releases:[{version:"0.30.0",security:false,severity:"none",notes:"x",notes_url:"u",auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",requires_ta_version:"0.28.0",proto_version:1,tarball:"t",sha256:$s,canary_ring:[]}]}' > "$OUT"
bash "$SIGN" --version 0.31.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" --no-baseline \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --notes v 2>"$ROOT/e18" >/dev/null \
  && bad "未签名本地 \$OUT(meta=9e9)竟被信任(可永久砖化)" || \
  { grep -qE "无 .minisig|拒绝静默信任" "$ROOT/e18" && ok "无 .minisig 的本地 \$OUT 被拒(默认不信任)" || bad "错误信息不对: $(cat "$ROOT/e18")"; }
# (b) 签好后篡改 counter(不重签)→ 验签失败被拒
rm -f "$OUT" "$OUT.minisig"
sign --version 0.30.0 --no-baseline --notes a >/dev/null 2>&1
jq '.metadata_version=999999999' "$OUT" > "$OUT.t" && mv "$OUT.t" "$OUT"   # 篡改,不重签 → .minisig 不匹配
bash "$SIGN" --version 0.31.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" --no-baseline \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --notes v 2>"$ROOT/e18b" >/dev/null \
  && bad "篡改的本地 \$OUT 竟被信任" || \
  { grep -q "验签失败" "$ROOT/e18b" && ok "篡改的本地 \$OUT 验签失败被拒" || bad "错误信息不对: $(cat "$ROOT/e18b")"; }

echo "== T19 撤销**最后一个** release → releases:[] 合法(镜像节点)+ 空告警(#196 R8 finding3)=="
rm -f "$OUT" "$OUT.minisig"
sign --version 0.30.0 --no-baseline --notes a >/dev/null 2>&1   # channel 只有 0.30.0
bash "$SIGN" --out "$OUT" --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --no-baseline --revoke 0.30.0 --notes r 2>"$ROOT/e19" >/dev/null && rc19=0 || rc19=1
[ "$rc19" = 0 ] && ok "撤销最后一版 rc=0(不再被 schema length>0 卡死)" || bad "撤销最后一版失败 rc=$rc19: $(cat "$ROOT/e19")"
[ "$(jq -r '.releases|length' "$OUT" 2>/dev/null)" = 0 ] && ok "releases:[](全撤销,与节点一致)" || bad "releases=$(jq -c .releases "$OUT")"
grep -q "无任何 release" "$ROOT/e19" && ok "空 releases 有告警" || bad "无空告警: $(cat "$ROOT/e19")"

echo
echo "结果: PASS=$PASS FAIL=$FAIL"
[ "$FAIL" = 0 ]
