#!/usr/bin/env bash
# 本地测试:社区节点自动更新器(无需硬件)。
# T1-T19 check 路径:升级成功 / 验签失败 / freeze / 回滚攻击 / 健康门→回滚 /
#   boot recovery / notify-only / TA-only-notify / 缺公钥 / 缺签名 / sha 不符 /
#   schema 非法 / tar symlink / canary 类型 / security 不被架空 / PIN / 兼容门 / 版本核对。
# T20-T28 Phase 1(apply CLI + 富通知 + 去重 + telegram hook):
#   T20 apply 越过 notify-only  T21 apply 含 TA 一律拒绝  T22 --allow-ta 已移除(未知选项)
#   T34/35 fetch 4xx/网络分流  T36 apply 网络硬失败  T37 creds export 到 hook
#   T38 apply 锁竞争硬失败  T39 notes 超长 schema 拒
#   T23 apply 不存在版本拒绝  T24 apply 降级拒绝  T25 apply 低于 floor 拒绝
#   T26 富通知(版本/severity/notes/应用提示)  T27 去重  T27b severity 升级重推
#   T28 notify-telegram.sh fail-safe + 发请求
set -uo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
UPDATER="$(cd "$HERE/../../deploy/updater" && pwd)/aastar-node-updater.sh"
SIGN="$(cd "$HERE/../../deploy/updater" && pwd)/sign-channel.sh"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

command -v minisign >/dev/null || { echo "SKIP: 需 minisign(brew install minisign)"; exit 0; }
command -v jq >/dev/null || { echo "SKIP: 需 jq"; exit 0; }

# 强制走 symlink CAS 回退路径,让锁相关用例在任意 host(含带 flock 的 Linux CI)结果一致。
# 生产走 flock(内核原语,无需在此单测);flock 路径由 T57 在有 flock 时单独 smoke。
# AU_TEST_MODE=1 是 split-brain 防护要求的显式测试标记(生产设 AU_LOCK_NO_FLOCK 且有 flock 会 die)。
export AU_LOCK_NO_FLOCK=1 AU_TEST_MODE=1

PASS=0; FAIL=0; SKIP=0
ok()   { echo -e "  \033[0;32mPASS\033[0m $*"; PASS=$((PASS+1)); }
bad()  { echo -e "  \033[0;31mFAIL\033[0m $*"; FAIL=$((FAIL+1)); }
skip() { echo -e "  \033[0;33mSKIP\033[0m $*"; SKIP=$((SKIP+1)); }   # 不计入 PASS,避免虚增(pr-daemon 八轮 #4)

ROOT="$(mktemp -d)" || { echo "FATAL: mktemp 失败"; exit 1; }
trap 'rm -rf "$ROOT"' EXIT
SERVER="$ROOT/server"; mkdir -p "$SERVER/channels" || { echo "FATAL: mkdir 失败"; exit 1; }

# ── 生成 minisign 密钥对(无密码,便于 CI/测试)──────────────────────
minisign -G -p "$ROOT/pub.key" -s "$ROOT/sec.key" -W >/dev/null 2>&1 \
  || { printf '\n\n' | minisign -G -p "$ROOT/pub.key" -s "$ROOT/sec.key" >/dev/null 2>&1; }
# 攻击者密钥对(用于伪造签名测试)
minisign -G -p "$ROOT/evil.pub" -s "$ROOT/evil.sec" -W >/dev/null 2>&1 \
  || { printf '\n\n' | minisign -G -p "$ROOT/evil.pub" -s "$ROOT/evil.sec" >/dev/null 2>&1; }
[ -f "$ROOT/pub.key" ] && [ -f "$ROOT/sec.key" ] && [ -f "$ROOT/evil.sec" ] \
  || { echo "FATAL: minisign 密钥生成失败"; exit 1; }

sha256_of() { if command -v sha256sum >/dev/null; then sha256sum "$1"|awk '{print $1}'; else shasum -a 256 "$1"|awk '{print $1}'; fi; }

# make_bundle <ver> [ta_marker] → 生成 tarball 到 server,echo sha256
make_bundle() {
  local ver="$1" ta="${2:-TA-$1}" d="$ROOT/b-$1"
  rm -rf "$d"; mkdir -p "$d/airaccount-node-$ver/kms"
  echo "kms-api-server $ver"        > "$d/airaccount-node-$ver/kms/kms-api-server"
  echo "$ta"                        > "$d/airaccount-node-$ver/kms/$UUID.ta"
  echo "$ver"                       > "$d/airaccount-node-$ver/kms/VERSION"
  tar -czf "$SERVER/airaccount-node-$ver.tar.gz" -C "$d" "airaccount-node-$ver"
  sha256_of "$SERVER/airaccount-node-$ver.tar.gz"
}

# write_manifest <metadata_version> <expires> <floor> <releases-json-array> [signkey]
write_manifest() {
  local mver="$1" exp="$2" floor="$3" rels="$4" key="${5:-$ROOT/sec.key}"
  jq -n --argjson m "$mver" --arg e "$exp" --arg f "$floor" --argjson r "$rels" \
    '{metadata_version:$m, generated_at:"2026-08-01T00:00:00Z", expires:$e, channel:"stable", rollback_floor:$f, releases:$r}' \
    > "$SERVER/channels/stable.json"
  MINISIGN_SECKEY="$key" "$SIGN" "$SERVER/channels/stable.json" >/dev/null 2>&1
}

# release_entry helper via jq inline in tests

# 每个 test 独立的节点根 + 状态
new_node() { # new_node <name> <baseline_ver> → echoes "AU_ROOT AU_STATE_DIR"
  local name="$1" ver="$2"
  local nr="$ROOT/node-$name" ns="$ROOT/state-$name"
  mkdir -p "$nr/releases/$ver" "$ns"
  echo "baseline" > "$nr/releases/$ver/VERSION"
  ln -sfn "releases/$ver" "$nr/current"
  ln -sfn "releases/$ver" "$nr/last-good"
  jq -n --arg v "$ver" '{seen_metadata_version:0,current:$v,previous:$v,pending:""}' > "$ns/state.json"
  echo "$nr $ns"
}

# mock hooks
cat > "$ROOT/mock-restart.sh" <<'SH'
#!/usr/bin/env bash
exit 0
SH
cat > "$ROOT/mock-notify.sh" <<SH
#!/usr/bin/env bash
echo "\$1 \$2" >> "$ROOT/notify.log"
SH
# 健康门:读 \$ROOT/health_result(0=pass,非0=fail)
cat > "$ROOT/mock-health.sh" <<SH
#!/usr/bin/env bash
exit \$(cat "$ROOT/health_result" 2>/dev/null || echo 0)
SH
chmod +x "$ROOT"/mock-*.sh

run_updater() { # run_updater <AU_ROOT> <AU_STATE_DIR> <cmd> [extra env assignments...]
  local nr="$1" ns="$2" cmd="$3"; shift 3
  env AU_ROOT="$nr" AU_STATE_DIR="$ns" AU_ENV_FILE="$ROOT/updater.env" \
      AU_PUBKEY="$ROOT/pub.key" AU_MANIFEST_BASE="file://$SERVER/channels" \
      AU_NODE_ID="testnode" \
      AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" \
      AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
      "$@" \
      bash "$UPDATER" "$cmd"
}

st() { jq -r --arg k "$2" '.[$k]' "$1/state.json"; }   # st <statedir> <key>
cur_link() { basename "$(readlink "$1/current")"; }     # cur_link <noderoot>

echo "0=pass health" > /dev/null; echo 0 > "$ROOT/health_result"

# ═══════════════════════════════════════════════════════════════════
echo "== T1 升级成功(security patch,policy=security,AUTO_UPDATE=on)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
CHANNEL=stable
UPDATE_POLICY=security
TA_AUTO_UPDATE=off
KEEP_RELEASES=3
ENV
read NR NS < <(new_node t1 0.28.0)
SHA="$(make_bundle 0.28.1 TA-0.28.0)"   # TA 未变(与 baseline 同)→ ta_changed=false
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.28.1.tar.gz" \
  '[{version:"0.28.1",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.28.0",tarball:$u,sha256:$s}]')"
write_manifest 2 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
echo 0 > "$ROOT/health_result"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  [ "$(st "$NS" current)" = "0.28.1" ] && ok "state.current=0.28.1" || bad "current=$(st "$NS" current)"
  [ "$(st "$NS" previous)" = "0.28.0" ] && ok "state.previous=0.28.0" || bad "previous=$(st "$NS" previous)"
  [ "$(st "$NS" pending)" = "" ] && ok "pending 清空" || bad "pending=$(st "$NS" pending)"
  [ "$(cur_link "$NR")" = "0.28.1" ] && ok "current 软链→0.28.1" || bad "link=$(cur_link "$NR")"
else bad "updater 退出非零(应成功)"; fi

# ═══════════════════════════════════════════════════════════════════
echo "== T2 验签失败拒绝(用攻击者私钥签)=="
read NR NS < <(new_node t2 0.28.0)
SHA="$(make_bundle 0.29.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 3 "2035-01-01T00:00:00Z" "0.0.0" "$REL" "$ROOT/evil.sec"   # 错误密钥
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "验签失败却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "验签失败,current 未变(0.28.0)" || bad "current=$(st "$NS" current)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T3 过期 manifest 拒绝(freeze attack)=="
read NR NS < <(new_node t3 0.28.0)
SHA="$(make_bundle 0.29.1)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.1.tar.gz" \
  '[{version:"0.29.1",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 4 "2000-01-01T00:00:00Z" "0.0.0" "$REL"   # 过期
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "过期 manifest 却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "过期拒绝,current 未变" || bad "current=$(st "$NS" current)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T4 回滚攻击拒绝(metadata_version 下降)=="
read NR NS < <(new_node t4 0.28.0)
# 先喂 metadata_version=5(无更高候选,只更新 seen)
write_manifest 5 "2035-01-01T00:00:00Z" "0.0.0" "[]"
run_updater "$NR" "$NS" check >/dev/null 2>&1
SEEN1="$(st "$NS" seen_metadata_version)"
[ "$SEEN1" = "5" ] && ok "seen 更新到 5" || bad "seen=$SEEN1"
# 再喂 metadata_version=3(旧)→ 应拒绝
SHA="$(make_bundle 0.30.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.30.0.tar.gz" \
  '[{version:"0.30.0",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 3 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "旧 metadata_version 却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "回滚攻击拒绝,current 未变" || bad "current=$(st "$NS" current)"
  [ "$(st "$NS" seen_metadata_version)" = "5" ] && ok "seen 保持 5(未被拉低)" || bad "seen=$(st "$NS" seen_metadata_version)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T5 健康门失败 → 自动回滚 =="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
UPDATE_POLICY=all
TA_AUTO_UPDATE=off
ENV
read NR NS < <(new_node t5 0.28.0)
SHA="$(make_bundle 0.29.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 6 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
echo 1 > "$ROOT/health_result"   # 健康门失败
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.28.0" ] && ok "健康门失败,current 回滚到 0.28.0" || bad "current=$(st "$NS" current)"
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "current 软链回滚→0.28.0" || bad "link=$(cur_link "$NR")"
[ "$(st "$NS" pending)" = "" ] && ok "pending 清空" || bad "pending=$(st "$NS" pending)"
[ -d "$NR/releases/0.29.0" ] && ok "0.29.0 已落盘(可复查)" || bad "0.29.0 目录缺失"
echo 0 > "$ROOT/health_result"

# ═══════════════════════════════════════════════════════════════════
echo "== T6 掉电中断 → boot recovery 回滚 =="
read NR NS < <(new_node t6 0.28.0)
# 模拟:更新到 0.29.0 时掉电 —— current 软链已切、pending 已设、但未提交
mkdir -p "$NR/releases/0.29.0"; echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"
ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:6,current:"0.28.0",previous:"0.28.0",pending:"0.29.0"}' > "$NS/state.json"
run_updater "$NR" "$NS" recovery >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "recovery:current 软链回滚→0.28.0" || bad "link=$(cur_link "$NR")"
[ "$(st "$NS" pending)" = "" ] && ok "recovery:pending 清空" || bad "pending=$(st "$NS" pending)"
[ "$(st "$NS" current)" = "0.28.0" ] && ok "recovery:state.current=0.28.0" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T6b 多次更新后掉电 → recovery 回到「已提交 current」而非 previous =="
read NR NS < <(new_node t6b 0.28.1)
# 版本链:0.28.0 → 0.28.1(已提交)→ 更新到 0.28.2 时掉电
mkdir -p "$NR/releases/0.28.0" "$NR/releases/0.28.2"
echo x > "$NR/releases/0.28.0/VERSION"; echo x > "$NR/releases/0.28.2/VERSION"
ln -sfn "releases/0.28.2" "$NR/current"        # 已切但未提交
ln -sfn "releases/0.28.1" "$NR/last-good"
jq -n '{seen_metadata_version:6,current:"0.28.1",previous:"0.28.0",pending:"0.28.2"}' > "$NS/state.json"
run_updater "$NR" "$NS" recovery >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.1" ] && ok "回到已提交 0.28.1(不是 previous 0.28.0)" || bad "link=$(cur_link "$NR")(过度回滚?)"
[ "$(st "$NS" current)" = "0.28.1" ] && ok "state.current=0.28.1" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T7 notify-only 默认不自动应用 =="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t7 0.28.0)
SHA="$(make_bundle 0.28.2 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.28.2.tar.gz" \
  '[{version:"0.28.2",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 7 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
: > "$ROOT/notify.log"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.28.0" ] && ok "notify-only,current 未变" || bad "current=$(st "$NS" current)"
grep -q "有新版本可用" "$ROOT/notify.log" && ok "发出了通知" || bad "未发通知"

# ═══════════════════════════════════════════════════════════════════
echo "== T8 TA 变更 + TA_AUTO_UPDATE=off → 只通知 =="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
UPDATE_POLICY=security
TA_AUTO_UPDATE=off
ENV
read NR NS < <(new_node t8 0.28.0)
SHA="$(make_bundle 0.28.3 TA-NEW)"   # TA 变了
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.28.3.tar.gz" \
  '[{version:"0.28.3",security:true,auto_apply_allowed:true,ta_changed:true,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 8 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.28.0" ] && ok "TA 变更未自动应用(current 未变)" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T9 缺验签公钥 → fail-closed 拒绝 =="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
UPDATE_POLICY=all
ENV
read NR NS < <(new_node t9 0.28.0)
SHA="$(make_bundle 0.29.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 9 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if env AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$ROOT/updater.env" \
      AU_PUBKEY="$ROOT/does-not-exist.pub" AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=testnode \
      AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
      bash "$UPDATER" check >/dev/null 2>&1; then
  bad "缺公钥却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "缺公钥,current 未变" || bad "current=$(st "$NS" current)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T10 缺 manifest 签名(.minisig)→ 拒绝 =="
read NR NS < <(new_node t10 0.28.0)
write_manifest 10 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
rm -f "$SERVER/channels/stable.json.minisig"     # 删掉签名
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "缺签名却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "缺签名,current 未变" || bad "current=$(st "$NS" current)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T11 tarball sha256 不匹配 → 拒绝 =="
read NR NS < <(new_node t11 0.28.0)
make_bundle 0.29.0 >/dev/null
BADSHA="0000000000000000000000000000000000000000000000000000000000000000"
REL="$(jq -n --arg s "$BADSHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 11 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "sha 不匹配却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "sha 不匹配,current 未变" || bad "current=$(st "$NS" current)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T12 schema 非法(version 非 semver)→ fail-closed 拒绝 =="
read NR NS < <(new_node t12 0.28.0)
SHA="$(make_bundle 0.29.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"latest",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 12 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "非法 schema 却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "非法 schema,current 未变" || bad "current=$(st "$NS" current)"
  [ "$(st "$NS" seen_metadata_version)" = "0" ] && ok "seen 未被污染(仍 0)" || bad "seen=$(st "$NS" seen_metadata_version)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T13 tarball 含 symlink 条目 → 拒绝(防 link 越界)=="
read NR NS < <(new_node t13 0.28.0)
d="$ROOT/bevil/airaccount-node-0.29.9/kms"; rm -rf "$ROOT/bevil"; mkdir -p "$d"
echo ca > "$d/kms-api-server"; echo ta > "$d/$UUID.ta"
ln -s /etc/passwd "$d/evil-link"                 # 恶意 symlink → 越界目标
tar -czf "$SERVER/airaccount-node-0.29.9.tar.gz" -C "$ROOT/bevil" airaccount-node-0.29.9
SHA="$(sha256_of "$SERVER/airaccount-node-0.29.9.tar.gz")"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.9.tar.gz" \
  '[{version:"0.29.9",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 13 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "含 symlink 的 tarball 却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "含 symlink,current 未变" || bad "current=$(st "$NS" current)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T14 canary_ring 类型非法 → schema 拒绝且 seen 不污染 =="
read NR NS < <(new_node t14 0.28.0)
SHA="$(make_bundle 0.29.0)"
# canary_ring 应为字符串数组,这里塞数字 → schema 应拒
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s,canary_ring:[123]}]')"
write_manifest 14 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "canary_ring 类型非法却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "非法 canary_ring,current 未变" || bad "current=$(st "$NS" current)"
  [ "$(st "$NS" seen_metadata_version)" = "0" ] && ok "seen 未被污染(仍 0)" || bad "seen=$(st "$NS" seen_metadata_version)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T15 security 策略不被更高的非安全版架空(PR#191 High-1)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
UPDATE_POLICY=security
TA_AUTO_UPDATE=off
ENV
read NR NS < <(new_node t15 0.28.0)
S1="$(make_bundle 0.28.1 TA-0.28.0)"   # 安全 patch
S2="$(make_bundle 0.29.0 TA-0.28.0)"   # 非安全 minor,版本更高
REL="$(jq -n --arg s1 "$S1" --arg u1 "file://$SERVER/airaccount-node-0.28.1.tar.gz" \
             --arg s2 "$S2" --arg u2 "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.28.1",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u1,sha256:$s1},
    {version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u2,sha256:$s2}]')"
write_manifest 15 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.28.1" ] && ok "自动打了安全补丁 0.28.1(未被更高的 0.29.0 挡住)" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T16 PIN_VERSION 不绕过 notify-only 总开关(PR#191 High-2)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
PIN_VERSION=0.29.0
ENV
read NR NS < <(new_node t16 0.28.0)
S="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$S" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 16 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.28.0" ] && ok "notify-only 下 PIN 匹配也不自动应用" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T17 PIN_VERSION + AUTO_UPDATE=on:匹配即应用(越过 security 策略)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
UPDATE_POLICY=security
PIN_VERSION=0.29.0
ENV
read NR NS < <(new_node t17 0.28.0)
S1="$(make_bundle 0.28.1 TA-0.28.0)"
S2="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s1 "$S1" --arg u1 "file://$SERVER/airaccount-node-0.28.1.tar.gz" \
             --arg s2 "$S2" --arg u2 "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.28.1",security:true,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u1,sha256:$s1},
    {version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u2,sha256:$s2}]')"
write_manifest 17 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.29.0" ] && ok "PIN=0.29.0 被应用(非安全也放行,因显式锁定)" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T18 requires_ta_version > 当前且不换 TA → 兼容性门只通知(PR#191 Medium)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
UPDATE_POLICY=all
TA_AUTO_UPDATE=off
ENV
read NR NS < <(new_node t18 0.28.0)
S="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$S" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,requires_ta_version:"0.29.0",min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 18 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.28.0" ] && ok "兼容性门拦下(current 未变)" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T19 健康门核对部署版本:restart 后仍旧版 → 回滚(PR#191 Medium)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
UPDATE_POLICY=all
ENV
read NR NS < <(new_node t19 0.28.0)
S="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$S" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 19 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
# 模拟「restart 后仍在跑旧版 0.28.0」:健康门只在期望版本==0.28.0 时才通过 → 部署 0.29.0 必失败
cat > "$ROOT/health-ver.sh" <<'SH'
#!/usr/bin/env bash
[ "${AU_EXPECT_VERSION:-}" = "0.28.0" ]
SH
chmod +x "$ROOT/health-ver.sh"
env AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$ROOT/updater.env" AU_PUBKEY="$ROOT/pub.key" \
    AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=testnode \
    AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/health-ver.sh" AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
    bash "$UPDATER" check >/dev/null 2>&1
[ "$(st "$NS" current)" = "0.28.0" ] && ok "版本核对失败→回滚(current 未变)" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
# Phase 1 新增:apply 子命令 + 富通知 + 去重 + telegram hook
# ═══════════════════════════════════════════════════════════════════

# 允许给 updater 传多个位置参数(apply <ver> [--allow-ta])
run_updater_args() { # <AU_ROOT> <AU_STATE_DIR> [env=val...] -- <updater args...>
  local nr="$1" ns="$2"; shift 2
  local envs=()
  while [ "${1:-}" != "--" ]; do envs+=("$1"); shift; done
  shift
  env AU_ROOT="$nr" AU_STATE_DIR="$ns" AU_ENV_FILE="$ROOT/updater.env" \
      AU_PUBKEY="$ROOT/pub.key" AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID="testnode" \
      AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" \
      AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
      ${envs[@]+"${envs[@]}"} \
      bash "$UPDATER" "$@"
}

echo "== T20 apply <ver>:notify-only 策略下也能显式手动应用 =="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t20 0.28.0)
SHA="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:false,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 20 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
echo 0 > "$ROOT/health_result"
if run_updater_args "$NR" "$NS" -- apply 0.29.0 >/dev/null 2>&1; then
  [ "$(st "$NS" current)" = "0.29.0" ] && ok "apply 0.29.0 成功(越过 notify-only 策略)" || bad "current=$(st "$NS" current)"
  [ "$(cur_link "$NR")" = "0.29.0" ] && ok "current 软链→0.29.0" || bad "link=$(cur_link "$NR")"
else bad "apply 退出非零(应成功)"; fi

echo "== T21 apply 含 TA 变更 → 一律拒绝(决策 D:TA 在线一键能力级砍掉,无 bypass)=="
read NR NS < <(new_node t21 0.28.0)
SHA="$(make_bundle 0.29.0 TA-NEW)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:true,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 21 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 0.29.0 >/dev/null 2>&1; then
  bad "TA 变更却成功(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "TA 变更被拒,current 未变" || bad "current=$(st "$NS" current)"
fi

echo "== T22 --allow-ta 选项已移除 → apply 传它报未知选项(不再有 TA bypass)=="
read NR NS < <(new_node t22 0.28.0)
SHA="$(make_bundle 0.29.0 TA-NEW)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:true,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 22 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 0.29.0 --allow-ta >/dev/null 2>&1; then
  bad "--allow-ta 竟被接受并应用了 TA(应报未知选项且不应用)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "--allow-ta 被拒(未知选项),TA 未应用" || bad "current=$(st "$NS" current)"
fi

echo "== T23 apply 不存在的版本 → 拒绝 =="
read NR NS < <(new_node t23 0.28.0)
SHA="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 23 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 9.9.9 >/dev/null 2>&1; then
  bad "不存在的版本却成功(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "不存在版本被拒" || bad "current=$(st "$NS" current)"
fi

echo "== T24 apply 降级(<= 当前)→ 拒绝 =="
read NR NS < <(new_node t24 0.29.0)
SHA="$(make_bundle 0.28.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.28.0.tar.gz" \
  '[{version:"0.28.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 24 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 0.28.0 >/dev/null 2>&1; then
  bad "降级却成功(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.29.0" ] && ok "降级被拒,current 未变" || bad "current=$(st "$NS" current)"
fi

echo "== T25 apply 低于 rollback_floor → 拒绝 =="
read NR NS < <(new_node t25 0.28.0)
SHA="$(make_bundle 0.28.5 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.28.5.tar.gz" \
  '[{version:"0.28.5",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 25 "2035-01-01T00:00:00Z" "0.29.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 0.28.5 >/dev/null 2>&1; then
  bad "低于 floor 却成功(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "低于 floor 被拒" || bad "current=$(st "$NS" current)"
fi

echo "== T26 富通知:含版本迁移 + 安全级别 + 变动摘要 =="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t26 0.28.0)
SHA="$(make_bundle 0.30.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.30.0.tar.gz" \
  '[{version:"0.30.0",security:true,severity:"high",notes:"修 stats XSS + portal 三语",auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 26 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
: > "$ROOT/notify.log"
run_updater "$NR" "$NS" check >/dev/null 2>&1
grep -q "有新版本可用:0.28.0 → 0.30.0" "$ROOT/notify.log" && ok "通知含版本迁移" || bad "通知缺版本迁移"
grep -q "HIGH" "$ROOT/notify.log" && ok "通知含安全级别 HIGH" || bad "通知缺 severity"
grep -q "修 stats XSS" "$ROOT/notify.log" && ok "通知含变动摘要 notes" || bad "通知缺 notes"
grep -q "aastar-node-updater apply 0.30.0" "$ROOT/notify.log" && ok "通知含应用命令提示" || bad "通知缺应用提示"

echo "== T27 通知去重:同更新第二次 check 不重复推 =="
: > "$ROOT/notify.log"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ ! -s "$ROOT/notify.log" ] && ok "同更新二次 check 去重(未重复通知)" || bad "重复通知: $(cat "$ROOT/notify.log")"

echo "== T27b severity 升级(同版本)→ 重推 =="
S2="$(make_bundle 0.30.0 TA-0.28.0)"   # 同版本同 hash
REL2="$(jq -n --arg s "$S2" --arg u "file://$SERVER/airaccount-node-0.30.0.tar.gz" \
  '[{version:"0.30.0",security:true,severity:"critical",notes:"升级为严重",auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 26 "2035-01-01T00:00:00Z" "0.0.0" "$REL2"   # mver 不变,仅 severity 变
: > "$ROOT/notify.log"
run_updater "$NR" "$NS" check >/dev/null 2>&1
grep -q "CRITICAL" "$ROOT/notify.log" && ok "severity 升级→重推(CRITICAL)" || bad "severity 升级未重推"

echo "== T28 notify-telegram.sh:未配置 exit0;配置+mock curl 发请求;token 不在 argv =="
NT="$(cd "$HERE/../../deploy/updater" && pwd)/notify-telegram.sh"
if env -u TELEGRAM_BOT_TOKEN -u TELEGRAM_CHAT_ID -u AAstarMonitorBot_TOKEN bash "$NT" info "t" >/dev/null 2>&1; then
  ok "未配置凭据时静默 exit 0(不阻断 updater)"
else bad "未配置却非零退出"; fi
# mock curl:记录 argv 与 stdin(URL 走 --config stdin,含 token)
cat > "$ROOT/mock-curl.sh" <<SH
#!/usr/bin/env bash
echo "\$@" > "$ROOT/curl-args.log"
cat > "$ROOT/curl-stdin.log"
exit 0
SH
chmod +x "$ROOT/mock-curl.sh"
: > "$ROOT/curl-args.log"; : > "$ROOT/curl-stdin.log"
if env TELEGRAM_BOT_TOKEN=TESTTOKEN TELEGRAM_CHAT_ID=123 CURL="$ROOT/mock-curl.sh" \
    bash "$NT" warn "有新版本可用:0.28.0 → 0.30.0" >/dev/null 2>&1; then
  ok "配置+发送成功 → exit 0"
else bad "配置+mock成功却非零退出"; fi
grep -q "sendMessage" "$ROOT/curl-stdin.log" && ok "sendMessage 端点(经 stdin --config)" || bad "未见 sendMessage"
grep -q "有新版本可用" "$ROOT/curl-args.log" && ok "argv 带消息内容" || bad "argv 缺消息内容"
grep -q "TESTTOKEN" "$ROOT/curl-args.log" && bad "token 出现在 argv(应只在 stdin)" || ok "token 不在 argv(仅 stdin,防 ps 泄露)"
# 发送失败(mock curl 非零)→ hook 返回非零(供 updater 去重判定)
cat > "$ROOT/mock-curl-fail.sh" <<'SH'
#!/usr/bin/env bash
cat >/dev/null; exit 7
SH
chmod +x "$ROOT/mock-curl-fail.sh"
if env TELEGRAM_BOT_TOKEN=T TELEGRAM_CHAT_ID=1 CURL="$ROOT/mock-curl-fail.sh" bash "$NT" info "x" >/dev/null 2>&1; then
  bad "发送失败却 exit 0(应非零,以便不写去重 key)"
else ok "发送失败 → 非零退出"; fi

echo "== T29 apply:bundle 实际含变化 TA 但 manifest 标 ta_changed=false → 拒绝(codex H2)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
# 造一个有「基线 TA」的节点(current 软链下有 .ta 可比对)
NR="$ROOT/node-t29"; NS="$ROOT/state-t29"; mkdir -p "$NR/releases/0.28.0" "$NS"
echo "kms 0.28.0" > "$NR/releases/0.28.0/kms-api-server"
echo "TA-OLD"      > "$NR/releases/0.28.0/$UUID.ta"
ln -sfn "releases/0.28.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:0,current:"0.28.0",previous:"0.28.0",pending:""}' > "$NS/state.json"
SHA="$(make_bundle 0.29.0 TA-DIFFERENT)"   # bundle TA 与基线不同
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 29 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 0.29.0 >/dev/null 2>&1; then
  bad "夹带变化 TA 却成功(应拒绝)"
else
  [ "$(cur_link "$NR")" = "0.28.0" ] && ok "夹带变化 TA 被 TA 内容门拒,current 未变" || bad "link=$(cur_link "$NR")"
fi
# 同样内容(TA 未真的变)→ 应通过
SHA2="$(make_bundle 0.29.1 TA-OLD)"       # bundle TA == 基线
REL2="$(jq -n --arg s "$SHA2" --arg u "file://$SERVER/airaccount-node-0.29.1.tar.gz" \
  '[{version:"0.29.1",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 30 "2035-01-01T00:00:00Z" "0.0.0" "$REL2"
echo 0 > "$ROOT/health_result"
if run_updater_args "$NR" "$NS" -- apply 0.29.1 >/dev/null 2>&1; then
  [ "$(cur_link "$NR")" = "0.29.1" ] && ok "TA 未真变(hash 同)→ 放行应用" || bad "link=$(cur_link "$NR")"
else bad "TA 未变却被拒(误报)"; fi

echo "== T30 apply 网络失败 → 硬失败非零(codex H1,不静默 exit0)=="
read NR NS < <(new_node t30 0.28.0)
# manifest 指向不存在的 file:// tarball
REL="$(jq -n --arg u "file://$SERVER/does-not-exist-0.31.0.tar.gz" \
  '[{version:"0.31.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:"0000000000000000000000000000000000000000000000000000000000000000"}]')"
write_manifest 31 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 0.31.0 >/dev/null 2>&1; then
  bad "tarball 拉不到却 exit 0(应硬失败)"
else ok "apply 下载失败 → 非零退出(不误判成功)"; fi

echo "== T31 schema:requires_ta_version 非 semver → 拒绝(codex M1)=="
read NR NS < <(new_node t31 0.28.0)
SHA="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,requires_ta_version:"latest",min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 32 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "requires_ta_version=latest 却通过(schema 应拒)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "非 semver requires_ta_version 被 schema 拒" || bad "current=$(st "$NS" current)"
fi

echo "== T32 通知未送达(hook 非零)→ 不写去重 key,下次重推(codex H3)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t32 0.28.0)
SHA="$(make_bundle 0.30.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.30.0.tar.gz" \
  '[{version:"0.30.0",security:true,severity:"high",notes:"x",auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 33 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
# 失败通知 hook(非零)
cat > "$ROOT/notify-fail.sh" <<'SH'
#!/usr/bin/env bash
exit 5
SH
chmod +x "$ROOT/notify-fail.sh"
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-fail.sh" -- check >/dev/null 2>&1
NK="$(st "$NS" notified_key)"   # st 用 .[$k],缺 key 返回字面 "null"
{ [ "$NK" = null ] || [ -z "$NK" ]; } && ok "hook 失败 → 未写 notified_key(下次会重推)" || bad "notified_key=$NK(不该写)"
# 换成会成功的 hook,再 check → 应重推并写 key
: > "$ROOT/notify.log"
run_updater "$NR" "$NS" check >/dev/null 2>&1
grep -q "有新版本可用" "$ROOT/notify.log" && ok "hook 恢复后重推(未被误去重)" || bad "未重推"
[ -n "$(st "$NS" notified_key)" ] && ok "送达成功 → 写 notified_key" || bad "成功却未写 key"

echo "== T33 apply:bundle 夹带额外/变化的第二个 TA(多 .ta)→ 仍拒绝(codex H1 复核)=="
NR="$ROOT/node-t33"; NS="$ROOT/state-t33"; mkdir -p "$NR/releases/0.28.0" "$NS"
echo "kms" > "$NR/releases/0.28.0/kms-api-server"
echo "TA-OLD" > "$NR/releases/0.28.0/$UUID.ta"
ln -sfn "releases/0.28.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:0,current:"0.28.0",previous:"0.28.0",pending:""}' > "$NS/state.json"
d="$ROOT/bmulti/airaccount-node-0.29.0/kms"; rm -rf "$ROOT/bmulti"; mkdir -p "$d"
echo "kms 0.29.0" > "$d/kms-api-server"
echo "TA-OLD"     > "$d/$UUID.ta"          # 与基线同
echo "EVIL-TA"    > "$d/evil-second.ta"    # 额外一个变化 TA(第一个可能先被拿到)
tar -czf "$SERVER/airaccount-node-0.29.0.tar.gz" -C "$ROOT/bmulti" airaccount-node-0.29.0
SHA="$(sha256_of "$SERVER/airaccount-node-0.29.0.tar.gz")"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 34 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater_args "$NR" "$NS" -- apply 0.29.0 >/dev/null 2>&1; then
  bad "多 TA(含变化)却成功(应拒绝)"
else
  [ "$(cur_link "$NR")" = "0.28.0" ] && ok "多 .ta 集合不一致被拒(不只看第一个)" || bad "link=$(cur_link "$NR")"
fi

echo "== T34 fetch:manifest 端点 HTTP 4xx(curl 22)→ 告警拒绝(不静默,#192)=="
read NR NS < <(new_node t34 0.28.0)
printf '#!/usr/bin/env bash\nexit 22\n' > "$ROOT/fetch-4xx.sh"; chmod +x "$ROOT/fetch-4xx.sh"
if run_updater_args "$NR" "$NS" AU_FETCH_CMD="$ROOT/fetch-4xx.sh" -- check >/dev/null 2>&1; then
  bad "HTTP 4xx 却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "4xx 拒绝,current 未变" || bad "current=$(st "$NS" current)"
fi

echo "== T35 fetch:manifest 网络失败(curl 7)→ check 静默退出 exit0(#192)=="
read NR NS < <(new_node t35 0.28.0)
printf '#!/usr/bin/env bash\nexit 7\n' > "$ROOT/fetch-net.sh"; chmod +x "$ROOT/fetch-net.sh"
run_updater_args "$NR" "$NS" AU_FETCH_CMD="$ROOT/fetch-net.sh" -- check >/dev/null 2>&1
RC=$?
[ "$RC" = 0 ] && ok "check 网络失败静默退出(exit 0)" || bad "exit=$RC(应 0)"

echo "== T36 fetch:apply 遇网络失败(curl 7)→ 硬失败非零(STRICT_FETCH,不静默)=="
read NR NS < <(new_node t36 0.28.0)
# 先给合法 manifest(apply 需先读到 manifest 选版本),再让 tarball 拉取网络失败
SHA="$(make_bundle 0.37.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.37.0.tar.gz" \
  '[{version:"0.37.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 37 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
# fetch mock:manifest(.json)正常放行走真实 file://,tarball 返回 7
cat > "$ROOT/fetch-mixed.sh" <<SH
#!/usr/bin/env bash
case "\$1" in
  *.tar.gz) exit 7 ;;                       # tarball 网络失败
  *) exec cp "\${1#file://}" "\$2" ;;        # 其它(manifest/sig)走真实文件
esac
SH
chmod +x "$ROOT/fetch-mixed.sh"
run_updater_args "$NR" "$NS" AU_FETCH_CMD="$ROOT/fetch-mixed.sh" -- apply 0.37.0 >/dev/null 2>&1
RC=$?
[ "$RC" != 0 ] && ok "apply 网络失败硬失败(exit=${RC} 非0,不误判成功)" || bad "apply 网络失败却 exit0"

echo "== T37 updater.env 的 TELEGRAM 凭据经 set -a 导出 → 通知 hook(子进程)看得到(pr-daemon #1)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
TELEGRAM_BOT_TOKEN=SEKRIT123
TELEGRAM_CHAT_ID=999
ENV
read NR NS < <(new_node t37 0.28.0)
SHA="$(make_bundle 0.30.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.30.0.tar.gz" \
  '[{version:"0.30.0",security:true,severity:"high",notes:"x",auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 37 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
# 通知 hook:把它进程环境里看到的 TELEGRAM_BOT_TOKEN 落盘
cat > "$ROOT/notify-envcheck.sh" <<SH
#!/usr/bin/env bash
echo "TOKEN=\${TELEGRAM_BOT_TOKEN:-<unset>}" > "$ROOT/seen-token.log"
SH
chmod +x "$ROOT/notify-envcheck.sh"
: > "$ROOT/seen-token.log"
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-envcheck.sh" -- check >/dev/null 2>&1
grep -q "TOKEN=SEKRIT123" "$ROOT/seen-token.log" && ok "hook 看到 updater.env 的 TELEGRAM_BOT_TOKEN(export 生效)" || bad "hook 未见 token: $(cat "$ROOT/seen-token.log")"

echo "== T38 apply 遇锁竞争(有活实例)→ 硬失败非零,不静默 no-op(pr-daemon #2)=="
read NR NS < <(new_node t38 0.28.0)
SHA="$(make_bundle 0.39.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.39.0.tar.gz" \
  '[{version:"0.39.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 39 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
ln -s "$$" "$NS/lock"     # 锁=symlink→活着的 pid(测试进程自身),模拟有实例在跑
run_updater_args "$NR" "$NS" -- apply 0.39.0 >/dev/null 2>&1
RC=$?
rm -f "$NS/lock"
{ [ "$RC" != 0 ] && [ "$(st "$NS" current)" = "0.28.0" ]; } && ok "apply 锁竞争硬失败(exit=${RC} 非0,current 未变)" || bad "exit=$RC current=$(st "$NS" current)(应非零且未变)"

echo "== T39 schema:notes 超长(>280)→ 拒绝(防超 Telegram 4096 无限重试,pr-daemon #5)=="
read NR NS < <(new_node t39 0.28.0)
SHA="$(make_bundle 0.29.0 TA-0.28.0)"
BIGNOTES="$(printf 'x%.0s' $(seq 1 400))"   # 400 字符
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" --arg n "$BIGNOTES" \
  '[{version:"0.29.0",security:true,notes:$n,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 40 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "超长 notes 却通过(schema 应拒)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "超长 notes 被 schema 拒" || bad "current=$(st "$NS" current)"
fi

echo "== T40 锁在 EXIT 时释放(cleanup rm -rf,不是 rmdir)→ 无泄漏(pr-daemon High)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t40 0.28.0)
SHA="$(make_bundle 0.41.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.41.0.tar.gz" \
  '[{version:"0.41.0",security:true,severity:"high",notes:"x",auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 41 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ ! -e "$NS/lock" ] && ok "check 结束后锁目录已删(rm -rf 生效,无泄漏)" || bad "锁目录残留 $NS/lock(泄漏)"

echo "== T41 cmd_recovery 也 source updater.env → 掉电回滚告警带 Telegram 凭据(pr-daemon #2)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
TELEGRAM_BOT_TOKEN=RECOV_TOK
TELEGRAM_CHAT_ID=1
ENV
NR="$ROOT/node-t41"; NS="$ROOT/state-t41"; mkdir -p "$NR/releases/0.28.0" "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.28.0/VERSION"; echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:6,current:"0.28.0",previous:"0.28.0",pending:"0.29.0"}' > "$NS/state.json"
cat > "$ROOT/notify-envcheck.sh" <<SH
#!/usr/bin/env bash
echo "TOKEN=\${TELEGRAM_BOT_TOKEN:-<unset>}" >> "$ROOT/recov-token.log"
SH
chmod +x "$ROOT/notify-envcheck.sh"
: > "$ROOT/recov-token.log"
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-envcheck.sh" -- recovery >/dev/null 2>&1
grep -q "TOKEN=RECOV_TOK" "$ROOT/recov-token.log" && ok "recovery 告警 hook 看到 TELEGRAM_BOT_TOKEN" || bad "recovery hook 未见 token: $(cat "$ROOT/recov-token.log")"

echo "== T42 兼容门用真实 TA 版本(非 CA 近似);TA 版本未知 → fail-closed(pr-daemon #3)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t42 0.30.0)
SHA="$(make_bundle 0.31.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.31.0.tar.gz" \
  '[{version:"0.31.0",security:false,auto_apply_allowed:true,ta_changed:false,requires_ta_version:"0.31.0",min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 42 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
# TA 版本未知(无 AU_TA_VERSION / TA_VERSION 文件)→ apply fail-closed 拒绝(不 fail-open)
if run_updater_args "$NR" "$NS" -- apply 0.31.0 >/dev/null 2>&1; then
  bad "TA 版本未知却放行(应 fail-closed 拒绝)"
else
  [ "$(cur_link "$NR")" = "0.30.0" ] && ok "TA 版本未知 → fail-closed 拒绝(不用 CA 版本近似)" || bad "link=$(cur_link "$NR")"
fi
# 设 AU_TA_VERSION=0.31.0(满足)→ 通过兼容门并应用
echo 0 > "$ROOT/health_result"
if run_updater_args "$NR" "$NS" AU_TA_VERSION=0.31.0 -- apply 0.31.0 >/dev/null 2>&1; then
  [ "$(cur_link "$NR")" = "0.31.0" ] && ok "AU_TA_VERSION 满足 → 兼容门放行应用" || bad "link=$(cur_link "$NR")"
else bad "AU_TA_VERSION 满足却被拒(误报)"; fi

echo "== T43 sha256 大写 manifest → 大小写不敏感比较,应放行(pr-daemon #4)=="
read NR NS < <(new_node t43 0.28.0)
SHA="$(make_bundle 0.32.0 TA-0.28.0)"
SHA_UP="$(printf '%s' "$SHA" | tr 'a-f' 'A-F')"
REL="$(jq -n --arg s "$SHA_UP" --arg u "file://$SERVER/airaccount-node-0.32.0.tar.gz" \
  '[{version:"0.32.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 43 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
echo 0 > "$ROOT/health_result"
if run_updater_args "$NR" "$NS" -- apply 0.32.0 >/dev/null 2>&1; then
  [ "$(cur_link "$NR")" = "0.32.0" ] && ok "大写 sha256 被接受(归一小写比较)" || bad "link=$(cur_link "$NR")"
else bad "大写 sha256 被误拒(fail-closed bug)"; fi

echo "== T44 TA 版本源在 release 树之外:current/TA_VERSION 不被采信(pr-daemon 三轮 #1)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t44 0.30.0)
echo "0.99.0" > "$NR/releases/0.30.0/TA_VERSION"   # 塞进 release 树(旧逻辑会误采信 → fail-open)
SHA="$(make_bundle 0.33.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.33.0.tar.gz" \
  '[{version:"0.33.0",security:false,auto_apply_allowed:true,ta_changed:false,requires_ta_version:"0.50.0",min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 44 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
# current/TA_VERSION=0.99.0 若被采信会满足 0.50.0 → 放行(fail-open);新逻辑不读它 → fail-closed 拒
if run_updater_args "$NR" "$NS" -- apply 0.33.0 >/dev/null 2>&1; then
  bad "current/TA_VERSION 被采信 → fail-open 放行(应忽略并 fail-closed)"
else
  [ "$(cur_link "$NR")" = "0.30.0" ] && ok "current/TA_VERSION 被忽略,fail-closed 拒绝(不 fail-open)" || bad "link=$(cur_link "$NR")"
fi
# 写状态目录的 ta-version(release 树之外)→ 被采信
echo "0.50.0" > "$NS/ta-version"
echo 0 > "$ROOT/health_result"
if run_updater_args "$NR" "$NS" -- apply 0.33.0 >/dev/null 2>&1; then
  [ "$(cur_link "$NR")" = "0.33.0" ] && ok "\$AU_STATE_DIR/ta-version(树外)被采信 → 放行" || bad "link=$(cur_link "$NR")"
else bad "状态目录 ta-version 满足却被拒"; fi

echo "== T45 apply 早期 die(缺版本参数)的告警也带凭据(main 最先 source,pr-daemon 三轮 #2)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
TELEGRAM_BOT_TOKEN=APPLY_TOK
TELEGRAM_CHAT_ID=1
ENV
read NR NS < <(new_node t45 0.28.0)
cat > "$ROOT/notify-envcheck.sh" <<SH
#!/usr/bin/env bash
echo "TOKEN=\${TELEGRAM_BOT_TOKEN:-<unset>}" > "$ROOT/apply-die-token.log"
SH
chmod +x "$ROOT/notify-envcheck.sh"
: > "$ROOT/apply-die-token.log"
# apply 不给版本 → main 的 npos 校验 die(早于任何 per-command source);main 已最先 source env
env AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$ROOT/updater.env" AU_PUBKEY="$ROOT/pub.key" \
    AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=testnode \
    AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" \
    AU_NOTIFY_CMD="$ROOT/notify-envcheck.sh" bash "$UPDATER" apply >/dev/null 2>&1
grep -q "TOKEN=APPLY_TOK" "$ROOT/apply-die-token.log" && ok "apply 早期 die 的告警 hook 看到凭据" || bad "apply die hook 未见 token: $(cat "$ROOT/apply-die-token.log")"

echo "== T46 recovery 告警入队 + 下次 check flush 投递(pr-daemon 三轮 #3)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
NR="$ROOT/node-t46"; NS="$ROOT/state-t46"; mkdir -p "$NR/releases/0.28.0" "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.28.0/VERSION"; echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:6,current:"0.28.0",previous:"0.28.0",pending:"0.29.0"}' > "$NS/state.json"
printf '#!/usr/bin/env bash\nexit 3\n' > "$ROOT/notify-fail.sh"; chmod +x "$ROOT/notify-fail.sh"
# recovery + 失败通知 → 应入队(网络没起的模拟)
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-fail.sh" -- recovery >/dev/null 2>&1
[ -f "$NS/pending-notify" ] && ok "recovery 告警投递失败 → 已入队 pending-notify" || bad "未入队(告警丢失)"
grep -q "boot recovery" "$NS/pending-notify" 2>/dev/null && ok "队列内容是掉电回滚告警" || bad "队列内容不对"
# 下次 check(网络已起,通知成功)→ flush 投递 + 删队列
cat > "$ROOT/notify-log.sh" <<SH
#!/usr/bin/env bash
echo "\$2" >> "$ROOT/flushed.log"
SH
chmod +x "$ROOT/notify-log.sh"
: > "$ROOT/flushed.log"
# 无更高候选(空 releases)让 check 只做 flush;metadata_version 需 > seen(6)
write_manifest 47 "2035-01-01T00:00:00Z" "0.0.0" "[]"
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-log.sh" -- check >/dev/null 2>&1
grep -q "boot recovery" "$ROOT/flushed.log" && ok "下次 check flush 投递了挂起告警" || bad "check 未投递挂起告警"
[ ! -f "$NS/pending-notify" ] && ok "投递成功后队列已清" || bad "队列未清(会重复投递)"

echo "== T47 stale 锁软链(→死 pid,崩溃遗留)→ 自动清理重取,不死锁(pr-daemon 四轮 #2)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t47 0.28.0)
write_manifest 48 "2035-01-01T00:00:00Z" "0.0.0" "[]"
ln -s 999999 "$NS/lock"   # 锁软链指向一个不存在的 pid(上次进程崩溃/掉电遗留)
run_updater "$NR" "$NS" check >/dev/null 2>&1
RC=$?
{ [ "$RC" = 0 ] && [ ! -e "$NS/lock" ]; } && ok "stale 锁软链被清理重取,check 跑完且释放锁(不死锁)" || bad "exit=$RC lock残留=$([ -e "$NS/lock" ] && echo yes)"

echo "== T48 无 updater.env → 各子命令仍 exit0(load_policy_env return 0,pr-daemon 四轮 #1 回归)=="
read NR NS < <(new_node t48 0.28.0)
NOENV="$ROOT/nonexistent-updater.env"; rm -f "$NOENV"
write_manifest 49 "2035-01-01T00:00:00Z" "0.0.0" "[]"    # 空候选,check 不会 die
for c in status check recovery; do
  env AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$NOENV" AU_PUBKEY="$ROOT/pub.key" \
      AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=testnode \
      AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
      bash "$UPDATER" "$c" >/dev/null 2>&1
  rc=$?
  [ "$rc" = 0 ] && ok "无 env 文件:$c exit 0" || bad "无 env 文件:$c exit=$rc(应 0,不静默 exit1)"
done

echo "== T49 并发:一实例持锁时,另一 apply 硬失败(真互斥,原子 symlink,pr-daemon 四轮 #2)=="
read NR NS < <(new_node t49 0.28.0)
write_manifest 50 "2035-01-01T00:00:00Z" "0.0.0" "[]"
# 慢 fetch:P1 的 check 在 acquire_lock 之后进 load_manifest 的 fetch 时 sleep,持锁约 2s
cat > "$ROOT/fetch-slow.sh" <<SH
#!/usr/bin/env bash
sleep 2
cp "\${1#file://}" "\$2"
SH
chmod +x "$ROOT/fetch-slow.sh"
run_updater_args "$NR" "$NS" AU_FETCH_CMD="$ROOT/fetch-slow.sh" -- check >/dev/null 2>&1 &
P1=$!
sleep 0.6                                   # 让 P1 先拿到锁并进入慢 fetch
run_updater_args "$NR" "$NS" -- apply 0.99.0 >/dev/null 2>&1   # P2:持锁期间抢 → 应硬失败
RC=$?
wait "$P1" 2>/dev/null
[ "$RC" != 0 ] && ok "P1 持锁时 P2 apply 硬失败(exit=${RC} 非0,真互斥)" || bad "P2 apply 竟成功(锁没互斥)"

echo "== T50 悬空 last-good(目标目录不存在)→ 不谎报回滚成功 + 入队不可恢复告警(pr-daemon 四轮 #3/#4)=="
NR="$ROOT/node-t50"; NS="$ROOT/state-t50"; mkdir -p "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"
ln -sfn "releases/0.1.0"  "$NR/last-good"    # 悬空:releases/0.1.0 不存在
jq -n '{seen_metadata_version:6,current:"0.5.0",previous:"0.5.0",pending:"0.29.0"}' > "$NS/state.json"  # current 0.5.0 也无对应目录
printf '#!/usr/bin/env bash\nexit 3\n' > "$ROOT/notify-fail.sh"; chmod +x "$ROOT/notify-fail.sh"
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-fail.sh" -- recovery >/dev/null 2>&1
RC=$?
[ "$RC" != 0 ] && ok "recovery 不可恢复 → 传播失败退出码(exit=${RC} 非0,不向 systemd 谎报成功,pr-daemon 五轮 #5)" || bad "recovery 不可恢复却 exit0(假成功)"
[ "$(cur_link "$NR")" = "0.29.0" ] && ok "current 未被指向悬空 0.1.0(不谎报成功)" || bad "current=$(cur_link "$NR")(误指悬空)"
[ "$(st "$NS" current)" != "0.1.0" ] && ok "state.current 未记成悬空 0.1.0" || bad "state.current=0.1.0(假成功)"
{ [ -f "$NS/pending-notify" ] && grep -q "不可恢复\|OOB" "$NS/pending-notify"; } && ok "不可恢复告警已入队(未走 fire-and-forget 丢失)" || bad "不可恢复告警丢失/未入队"

echo "== T51 锁路径残留老格式目录(升级前 mkdir 锁)→ 一次性迁移,锁不变永久 no-op(pr-daemon 五轮 #1)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=notify-only
UPDATE_POLICY=security
ENV
read NR NS < <(new_node t51 0.28.0)
write_manifest 51 "2035-01-01T00:00:00Z" "0.0.0" "[]"
mkdir -p "$NS/lock"; echo old > "$NS/lock/pid"    # 老 mkdir 锁遗留的**目录**(非软链)
run_updater "$NR" "$NS" check >/dev/null 2>&1; RC=$?
# 若未迁移:ln -s 落进目录 → check 假持锁跑完但目录还在(mutual exclusion 永久失效)
{ [ "$RC" = 0 ] && [ ! -e "$NS/lock" ]; } && ok "老格式目录被迁移清除,锁恢复正常(非永久 no-op)" || bad "exit=$RC lock残留=$([ -e "$NS/lock" ] && echo yes)"
# 迁移后能真互斥:再放一个活锁软链 → apply 应硬失败
ln -s "$$" "$NS/lock"
run_updater_args "$NR" "$NS" -- apply 0.99.0 >/dev/null 2>&1; RC2=$?
rm -f "$NS/lock"
[ "$RC2" != 0 ] && ok "迁移后活锁软链能挡住 apply(真互斥恢复)" || bad "迁移后仍不互斥"

echo "== T52 锁软链指向非法 pid(0 / 负 / 垃圾)→ 当 stale 抢占,不误判存活(pr-daemon 五轮 #3)=="
read NR NS < <(new_node t52 0.28.0)
write_manifest 52 "2035-01-01T00:00:00Z" "0.0.0" "[]"
for badpid in 0 00 000 garbage; do   # 0 及零填充 00/000(kill -0 00/000 命中进程组 0 会误判存活,#8);garbage 非数字
  ln -sfn "$badpid" "$NS/lock"
  run_updater "$NR" "$NS" check >/dev/null 2>&1; RC=$?
  { [ "$RC" = 0 ] && [ ! -e "$NS/lock" ]; } && ok "非法 pid '$badpid' 当 stale 被抢占(check 跑完)" || bad "非法 pid '$badpid' 误判存活 exit=$RC"
done

echo "== T53 recovery 只清 stale 锁,不删活锁(pr-daemon 五轮 #4)=="
read NR NS < <(new_node t53 0.28.0)   # 无 pending → recovery 只做锁清理判断
ln -s "$$" "$NS/lock"                  # 活锁(测试进程 pid)
run_updater "$NR" "$NS" recovery >/dev/null 2>&1
[ -L "$NS/lock" ] && [ "$(readlink "$NS/lock")" = "$$" ] && ok "recovery 保留活锁(未盲删,避免双持)" || bad "recovery 删了活锁"
rm -f "$NS/lock"
# 反面:stale(死 pid)锁 → recovery 应清掉
ln -sfn 999999 "$NS/lock"
run_updater "$NR" "$NS" recovery >/dev/null 2>&1
[ ! -e "$NS/lock" ] && ok "recovery 清掉了 stale 死 pid 锁" || bad "stale 锁未清"

echo "== T54 recovery 也持锁:活实例在跑时 recovery 不 lock-free 改 state/软链(pr-daemon 六轮 #1)=="
NR="$ROOT/node-t54"; NS="$ROOT/state-t54"; mkdir -p "$NR/releases/0.28.0" "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.28.0/VERSION"; echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:6,current:"0.28.0",previous:"0.28.0",pending:"0.29.0"}' > "$NS/state.json"
ln -s "$$" "$NS/lock"    # 活锁(测试进程 pid),模拟有 check/apply 正在跑
run_updater "$NR" "$NS" recovery >/dev/null 2>&1; RC=$?
rm -f "$NS/lock"
[ "$RC" != 0 ] && ok "recovery 锁竞争硬失败(exit=${RC} 非0,systemd 可见;不静默跳过回滚,七轮 #1)" || bad "recovery 竞争却 exit0(静默跳过回滚)"
[ "$(cur_link "$NR")" = "0.29.0" ] && ok "活实例持锁时 recovery 未改 current(先持锁,竞争则让位)" || bad "current=$(cur_link "$NR")(被 lock-free 改了)"
[ "$(st "$NS" pending)" = "0.29.0" ] && ok "pending 未被 recovery 改动(让位)" || bad "pending 被改=$(st "$NS" pending)"

echo "== T55 老格式锁目录 pid 还活着(升级切换中)→ 不盲删,当竞争(pr-daemon 六轮 #5)=="
read NR NS < <(new_node t55 0.28.0)
mkdir -p "$NS/lock"; echo "$$" > "$NS/lock/pid"    # 老格式目录 + 活 pid(测试进程)
run_updater_args "$NR" "$NS" -- apply 0.99.0 >/dev/null 2>&1; RC=$?
D=no; [ -d "$NS/lock" ] && D=yes
rm -rf "$NS/lock"
{ [ "$RC" != 0 ] && [ "$D" = yes ]; } && ok "老格式锁活 pid → apply 硬失败且目录未被盲删" || bad "RC=$RC dir还在=$D"

echo "== T56 不可恢复不清 pending → 每次 boot 都重报失败(pr-daemon 六轮 #4)=="
NR="$ROOT/node-t56"; NS="$ROOT/state-t56"; mkdir -p "$NR/releases/0.29.0" "$NS"
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.1.0" "$NR/last-good"   # 悬空
jq -n '{seen_metadata_version:6,current:"0.5.0",previous:"0.5.0",pending:"0.29.0"}' > "$NS/state.json"
printf '#!/usr/bin/env bash\nexit 3\n' > "$ROOT/notify-fail.sh"; chmod +x "$ROOT/notify-fail.sh"
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-fail.sh" -- recovery >/dev/null 2>&1; RC1=$?
[ "$(st "$NS" pending)" = "0.29.0" ] && ok "第一次 boot 不可恢复:pending 未清(留作后续重报)" || bad "pending 被清=$(st "$NS" pending)"
run_updater_args "$NR" "$NS" AU_NOTIFY_CMD="$ROOT/notify-fail.sh" -- recovery >/dev/null 2>&1; RC2=$?
{ [ "$RC1" != 0 ] && [ "$RC2" != 0 ]; } && ok "第二次 boot 仍 exit 非0 重报(不掩盖持久失败)" || bad "RC1=$RC1 RC2=$RC2"

echo "== T57 生产 flock 路径:真互斥 + 非空 stderr(用真候选,current 作判别,pr-daemon 八轮 #3)=="
if command -v flock >/dev/null 2>&1; then
  read NR NS < <(new_node t57 0.28.0)
  S="$(make_bundle 0.60.0 TA-0.28.0)"   # **真** applicable release:无竞争时 apply 会成功
  REL="$(jq -n --arg s "$S" --arg u "file://$SERVER/airaccount-node-0.60.0.tar.gz" \
    '[{version:"0.60.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
  write_manifest 60 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
  echo 0 > "$ROOT/health_result"
  ( exec 9>"$NS/lock.flock"; flock -n 9 && sleep 3 ) &    # 外部持 flock 3s
  HOLDER=$!; sleep 0.5
  env -u AU_LOCK_NO_FLOCK AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$ROOT/updater.env" AU_PUBKEY="$ROOT/pub.key" \
      AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=t \
      AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
      bash "$UPDATER" apply 0.60.0 >/dev/null 2>"$ROOT/t57.stderr"; RC=$?
  wait "$HOLDER" 2>/dev/null
  # 判别:flock 被占 → apply 应硬失败**且 current 没被换到 0.60.0**(若锁没互斥,apply 会成功换掉)
  { [ "$RC" != 0 ] && [ "$(cur_link "$NR")" = "0.28.0" ]; } && ok "flock 被占 → apply 硬失败且 current 未变(真互斥)" || bad "RC=$RC current=$(cur_link "$NR")"
  # stderr 非空(八轮 #1:exec 2>/dev/null 曾把 stderr 永久打死 → 这里必须能看到竞争红字)
  [ -s "$ROOT/t57.stderr" ] && grep -q "持锁\|实例" "$ROOT/t57.stderr" && ok "flock 竞争有 stderr 输出(未被 exec 永久重定向吞掉)" || bad "stderr 空/无竞争信息: $(cat "$ROOT/t57.stderr")"
else
  skip "flock 真互斥(本 host 无 flock,生产 Linux 有)"; skip "flock stderr 非空(同上)"
fi

echo "== T58 split-brain 防护:有 flock 时设 AU_LOCK_NO_FLOCK 但无 AU_TEST_MODE → die 且报 split-brain(八轮 #7)=="
if command -v flock >/dev/null 2>&1; then
  read NR NS < <(new_node t58 0.28.0)
  write_manifest 61 "2035-01-01T00:00:00Z" "0.0.0" "[]"
  if env -u AU_TEST_MODE AU_LOCK_NO_FLOCK=1 AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$ROOT/updater.env" \
        AU_PUBKEY="$ROOT/pub.key" AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=t \
        AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
        bash "$UPDATER" check >/dev/null 2>"$ROOT/t58.stderr"; then
    bad "生产(有 flock)no-flock 无 TEST_MODE 却放行(应 die 防 split-brain)"
  else grep -q "split-brain" "$ROOT/t58.stderr" && ok "生产 no-flock 无 AU_TEST_MODE → die 且明确 split-brain" || bad "die 了但非 split-brain 原因: $(cat "$ROOT/t58.stderr")"; fi
else
  skip "split-brain 防护(本 host 无 flock)"
fi

echo "== T59 面板 rollback verb:apply 成功后(pending 空)无条件回到 previous + 真实 restart =="
# 这正是 recovery 会静默 no-op 的状态(pending 空);面板按钮必须真回滚。
NR="$ROOT/n59"; NS="$ROOT/s59"; mkdir -p "$NR/releases/0.28.0" "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.28.0/VERSION"; echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:9,current:"0.29.0",previous:"0.28.0",pending:""}' > "$NS/state.json"
: > "$ROOT/rb-restart.log"
cat > "$ROOT/rec-restart.sh" <<SH
#!/usr/bin/env bash
echo restarted >> "$ROOT/rb-restart.log"
SH
chmod +x "$ROOT/rec-restart.sh"
# 先证明:同状态下 recovery 是空操作(pending 空 → current 不变)
run_updater "$NR" "$NS" recovery >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.29.0" ] && ok "recovery 在 pending 空时确为 no-op(current 未动)" || bad "recovery 竟动了 current=$(cur_link "$NR")"
# 再证明:rollback verb 真回滚
run_updater "$NR" "$NS" rollback AU_RESTART_CMD="$ROOT/rec-restart.sh" >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "rollback:current 软链→0.28.0(真回滚,非空操作)" || bad "link=$(cur_link "$NR")"
[ "$(st "$NS" current)" = "0.28.0" ] && ok "rollback:state.current=0.28.0" || bad "current=$(st "$NS" current)"
[ -z "$(st "$NS" previous)" ] && ok "rollback:previous 清空(去 toggle,二次点不会装回坏版本)" || bad "previous 未清=$(st "$NS" previous)"
[ "$(jq -r '(.denied//[])|index("0.29.0")' "$NS/state.json")" != "null" ] && ok "rollback:坏版本 0.29.0 进 denied(阻 6h 自动重装)" || bad "denied 未含 0.29.0=$(jq -c '.denied' "$NS/state.json")"
[ -s "$ROOT/rb-restart.log" ] && ok "rollback:执行了真实 restart(非 recovery 的 AU_RESTART_CMD=true)" || bad "rollback 未 restart"

echo "== T60 rollback 无 previous → 硬失败(die),不静默成功、不动 current =="
NR="$ROOT/n60"; NS="$ROOT/s60"; mkdir -p "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.29.0/VERSION"; ln -sfn "releases/0.29.0" "$NR/current"
jq -n '{seen_metadata_version:9,current:"0.29.0",previous:"",pending:""}' > "$NS/state.json"
if run_updater "$NR" "$NS" rollback >/dev/null 2>&1; then bad "无 previous 竟 exit 0(应 die)"; else ok "无 previous → 硬失败(exit 非0)"; fi
[ "$(cur_link "$NR")" = "0.29.0" ] && ok "无 previous:current 未被动(0.29.0)" || bad "current 被动=$(cur_link "$NR")"

# ═══════════════════════════════════════════════════════════════════
echo "== T61 连按两次面板 rollback:第二次无 previous → die,current 不 toggle 回坏版本(#195 R4)=="
NR="$ROOT/n61"; NS="$ROOT/s61"; mkdir -p "$NR/releases/0.28.0" "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.28.0/VERSION"; echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:9,current:"0.29.0",previous:"0.28.0",pending:""}' > "$NS/state.json"
run_updater "$NR" "$NS" rollback AU_RESTART_CMD=true >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "第一次 rollback→0.28.0" || bad "link=$(cur_link "$NR")"
if run_updater "$NR" "$NS" rollback AU_RESTART_CMD=true >/dev/null 2>&1; then bad "第二次 rollback 竟 exit0(应 die,previous 已清)"; else ok "第二次 rollback 硬失败(previous 已清,不 toggle)"; fi
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "第二次后 current 仍 0.28.0(未 toggle 回坏版本 0.29.0)" || bad "current toggle 回=$(cur_link "$NR")"

# ═══════════════════════════════════════════════════════════════════
echo "== T62 rollback 后 check:被 deny 的坏版本不再自动重装(#195 High:否则 timer 6h 重装)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
CHANNEL=stable
UPDATE_POLICY=all
TA_AUTO_UPDATE=off
KEEP_RELEASES=3
ENV
NR="$ROOT/n62"; NS="$ROOT/s62"; mkdir -p "$NR/releases/0.28.0" "$NR/releases/0.29.0" "$NS"
echo x > "$NR/releases/0.28.0/VERSION"; echo x > "$NR/releases/0.29.0/VERSION"
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
jq -n '{seen_metadata_version:9,current:"0.29.0",previous:"0.28.0",pending:""}' > "$NS/state.json"
run_updater "$NR" "$NS" rollback AU_RESTART_CMD=true >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "T62 前置:回滚到 0.28.0(deny 0.29.0)" || bad "link=$(cur_link "$NR")"
# manifest 仍提供 0.29.0(UPDATE_POLICY=all 本会自动装)——但它已被 deny → 不装
SHA="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 10 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
echo 0 > "$ROOT/health_result"
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "check 未重装被 deny 的 0.29.0(current 软链仍 0.28.0)" || bad "坏版本被重装=$(cur_link "$NR")"
[ "$(st "$NS" current)" = "0.28.0" ] && ok "state.current 仍 0.28.0" || bad "current=$(st "$NS" current)"

# ═══════════════════════════════════════════════════════════════════
echo "== T63 pending 非空时 rollback:目标取 current(非 previous),不多退一级(#195 Medium)=="
NR="$ROOT/n63"; NS="$ROOT/s63"; mkdir -p "$NR/releases/0.27.0" "$NR/releases/0.28.0" "$NR/releases/0.29.0" "$NS"
for v in 0.27.0 0.28.0 0.29.0; do echo x > "$NR/releases/$v/VERSION"; done
ln -sfn "releases/0.29.0" "$NR/current"; ln -sfn "releases/0.28.0" "$NR/last-good"
# 中断的 apply:current=0.28.0(已提交),previous=0.27.0,pending=0.29.0(半装,软链已切 0.29.0)
jq -n '{seen_metadata_version:9,current:"0.28.0",previous:"0.27.0",pending:"0.29.0"}' > "$NS/state.json"
run_updater "$NR" "$NS" rollback AU_RESTART_CMD=true >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "pending 时目标=current(0.28.0),未多退到 previous(0.27.0)" || bad "link=$(cur_link "$NR")"
[ "$(st "$NS" pending)" = "" ] && ok "pending 已清(中断的 apply 已解决)" || bad "pending=$(st "$NS" pending)"
[ "$(jq -r '(.denied//[])|index("0.29.0")' "$NS/state.json")" != "null" ] && ok "坏版本 pending=0.29.0 进 denied" || bad "denied=$(jq -c '.denied' "$NS/state.json")"
[ "$(st "$NS" previous)" = "0.27.0" ] && ok "previous 未动(仍 0.27.0,pending 分支不清 previous)" || bad "previous=$(st "$NS" previous)"

# ═══════════════════════════════════════════════════════════════════
echo "== T64 list-candidates:只读列候选(不安装),标 action/denied(#195 finding4 只读动词)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
CHANNEL=stable
UPDATE_POLICY=all
TA_AUTO_UPDATE=off
KEEP_RELEASES=3
ENV
read NR NS < <(new_node t64 0.28.0)
S1="$(make_bundle 0.29.0 TA-0.28.0)"; S2="$(make_bundle 0.30.0 TA-0.28.0)"
REL="$(jq -n --arg s1 "$S1" --arg s2 "$S2" --arg u1 "file://$SERVER/airaccount-node-0.29.0.tar.gz" --arg u2 "file://$SERVER/airaccount-node-0.30.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u1,sha256:$s1},
    {version:"0.30.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u2,sha256:$s2}]')"
write_manifest 11 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
jq '.denied=["0.30.0"]' "$NS/state.json" > "$NS/state.json.t" && mv "$NS/state.json.t" "$NS/state.json"   # 预先 deny 0.30.0
OUT="$(run_updater_args "$NR" "$NS" -- list-candidates 2>/dev/null)"; rc=$?
[ "$rc" = 0 ] && ok "list-candidates rc=0(helper 靠退出码判成功/失败,不能吃掉)" || bad "list-candidates rc=$rc(应 0)"
echo "$OUT" | jq -e . >/dev/null 2>&1 && ok "list-candidates 输出合法 JSON(stdout 干净)" || bad "非法 JSON: $OUT"
[ "$(echo "$OUT" | jq -r '.current')" = "0.28.0" ] && ok "current=0.28.0" || bad "current=$(echo "$OUT" | jq -r '.current')"
[ "$(echo "$OUT" | jq -r '.candidates|length')" = "2" ] && ok "列出 2 个候选" || bad "候选数=$(echo "$OUT" | jq -r '.candidates|length')"
[ "$(echo "$OUT" | jq -r '.candidates[]|select(.version=="0.30.0").action')" = "denied" ] && ok "0.30.0 标 action=denied" || bad "0.30.0 action=$(echo "$OUT" | jq -r '.candidates[]|select(.version=="0.30.0").action')"
[ "$(echo "$OUT" | jq -r '.candidates[]|select(.version=="0.29.0").action')" = "apply" ] && ok "0.29.0 标 action=apply(policy=all)" || bad "0.29.0 action=$(echo "$OUT" | jq -r '.candidates[]|select(.version=="0.29.0").action')"
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "list-candidates 未安装(current 软链仍 0.28.0)" || bad "竟安装了=$(cur_link "$NR")"

# ═══════════════════════════════════════════════════════════════════
echo "== T65 list-candidates **零候选** → 仍 rc=0 + 合法 JSON(#195 R5 finding2:『已是最新』最常见)=="
read NR NS < <(new_node t65 0.30.0)
write_manifest 12 "2035-01-01T00:00:00Z" "0.0.0" "[]"   # 无 release → 零候选
OUT="$(run_updater_args "$NR" "$NS" -- list-candidates 2>/dev/null)"; rc=$?
[ "$rc" = 0 ] && ok "零候选 rc=0(不是 exit1)" || bad "零候选 rc=$rc(应 0)"
echo "$OUT" | jq -e '.candidates==[] and .current=="0.30.0"' >/dev/null 2>&1 && ok "零候选 candidates=[] + current 正确" || bad "零候选 JSON 不对: $OUT"

# ═══════════════════════════════════════════════════════════════════
echo "== T66 自动 apply 健康门失败 → deny 坏版本,下次 check 不重装(#195 R5 finding3:自动路径)=="
cat > "$ROOT/updater.env" <<'ENV'
AUTO_UPDATE=on
CHANNEL=stable
UPDATE_POLICY=all
TA_AUTO_UPDATE=off
KEEP_RELEASES=3
ENV
read NR NS < <(new_node t66 0.28.0)
SHA="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$SHA" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:false,auto_apply_allowed:true,ta_changed:false,min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 13 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
echo 1 > "$ROOT/health_result"    # 健康门失败 → 自动回滚
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "健康门失败 → 自动回滚到 0.28.0" || bad "未回滚=$(cur_link "$NR")"
[ "$(jq -r '(.denied//[])|index("0.29.0")' "$NS/state.json")" != "null" ] && ok "坏版本 0.29.0 进 denied(**自动**路径)" || bad "denied 未含 0.29.0=$(jq -c '.denied' "$NS/state.json")"
echo 0 > "$ROOT/health_result"    # 即便健康门恢复,被 deny 的也不该重装
run_updater "$NR" "$NS" check >/dev/null 2>&1
[ "$(cur_link "$NR")" = "0.28.0" ] && ok "下次 check 不重装被 deny 的 0.29.0(终结 6h 循环)" || bad "坏版本被重装=$(cur_link "$NR")"

# ═══════════════════════════════════════════════════════════════════
echo ""
echo "结果: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
[ "$FAIL" -eq 0 ]
