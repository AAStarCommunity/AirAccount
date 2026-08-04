#!/usr/bin/env bash
# test-release-sign.sh —— release-sign.sh(发版签发工具)测试。
# 重点:①signer 产物必须能过节点 aastar-node-updater.sh 的 load_manifest(防两份手写 schema 漂移);
#       ②--dry-run 绝不触碰 $OUT;③多行/超长 notes 必须被 signer 拒;④channel 越目录拒;
#       ⑤metadata_version 单调(读回基线)。
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
SIGN="$(cd "$HERE/../../deploy/updater" && pwd)/release-sign.sh"
UPDATER="$(cd "$HERE/../../deploy/updater" && pwd)/aastar-node-updater.sh"

command -v minisign >/dev/null || { echo "SKIP: 需 minisign"; exit 0; }
command -v jq >/dev/null || { echo "SKIP: 需 jq"; exit 0; }

PASS=0; FAIL=0
ok()   { echo "  PASS $*"; PASS=$((PASS+1)); }
bad()  { echo "  FAIL $*"; FAIL=$((FAIL+1)); }

ROOT="$(mktemp -d)" || exit 1
trap 'rm -rf "$ROOT"' EXIT
# 测试密钥(无密码)
minisign -G -p "$ROOT/pub.key" -s "$ROOT/sec.key" -W >/dev/null 2>&1 \
  || printf '\n\n' | minisign -G -p "$ROOT/pub.key" -s "$ROOT/sec.key" >/dev/null 2>&1
echo "dummy node bundle" > "$ROOT/airaccount-node-v0.30.0.tar.gz"
OUT="$ROOT/stable.json"

sign() { bash "$SIGN" --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --no-baseline "$@"; }

echo "== T1 signer 真实产物必须过节点 load_manifest(核心防漂移)=="
if sign --version 0.30.0 --severity high --security --notes "安全补丁" >/dev/null 2>&1 \
   && [ -f "$OUT" ] && [ -f "$OUT.minisig" ]; then
  # 用节点 updater check(notify-only)跑真实 load_manifest:file:// 拉本地已签 manifest + 测试公钥验签。
  st="$ROOT/state"; mkdir -p "$st"
  if AU_MANIFEST_BASE="file://$ROOT" AU_PUBKEY="$ROOT/pub.key" AU_STATE_DIR="$st" \
     AU_ROOT="$ROOT/opt" AU_NOTIFY_CMD=true AU_LOCK_NO_FLOCK=1 AU_TEST_MODE=1 \
     bash "$UPDATER" check >/dev/null 2>&1; then
    ok "signer 产物通过节点 load_manifest(验签+schema+新鲜度+防回滚)"
  else
    bad "signer 产物**未过**节点 load_manifest —— 两份 schema 已漂移!"
  fi
else
  bad "signer 未能产出 stable.json / .minisig"
fi

echo "== T2 --dry-run 绝不触碰 \$OUT =="
rm -f "$OUT" "$OUT.minisig"
sign --version 0.31.0 --dry-run >/dev/null 2>&1
{ [ ! -f "$OUT" ] && [ ! -f "$OUT.minisig" ]; } && ok "dry-run 未写 \$OUT/.minisig" || bad "dry-run 竟落盘"

echo "== T3 多行/超长 notes 必须被 signer 拒(否则每个节点 fail-closed 拒 manifest)=="
if sign --version 0.31.0 --dry-run --notes $'a\nb' >/dev/null 2>&1; then bad "多行 notes 未拒"; else ok "多行 notes 被拒"; fi
big="$(printf 'x%.0s' $(seq 1 300))"
if sign --version 0.31.0 --dry-run --notes "$big" >/dev/null 2>&1; then bad "超长 notes 未拒"; else ok "超长 notes 被拒"; fi

echo "== T4 channel 越目录必须拒 =="
if bash "$SIGN" --version 0.30.0 --tarball "$ROOT/airaccount-node-v0.30.0.tar.gz" \
   --channel "../../pwn" --no-baseline --dry-run >/dev/null 2>&1; then bad "越目录 channel 未拒"; else ok "越目录 channel 被拒"; fi

echo "== T5 metadata_version 单调(叠加第二版应 =2,同版重签仍单调)=="
sign --version 0.30.0 --notes v1 >/dev/null 2>&1
echo "bundle31" > "$ROOT/airaccount-node-v0.31.0.tar.gz"
bash "$SIGN" --version 0.31.0 --tarball "$ROOT/airaccount-node-v0.31.0.tar.gz" --out "$OUT" \
  --seckey "$ROOT/sec.key" --pubkey "$ROOT/pub.key" --no-baseline --notes v2 >/dev/null 2>&1
mv2="$(jq -r '.metadata_version' "$OUT")"
nrel="$(jq -r '.releases|length' "$OUT")"
{ [ "$mv2" -ge 2 ] && [ "$nrel" = 2 ]; } && ok "metadata_version=$mv2(单调)releases=$nrel(累积)" || bad "metadata_version=$mv2 releases=$nrel 异常"

echo
echo "结果: PASS=$PASS FAIL=$FAIL"
[ "$FAIL" = 0 ]
