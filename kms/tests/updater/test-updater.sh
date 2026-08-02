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

PASS=0; FAIL=0
ok()   { echo -e "  \033[0;32mPASS\033[0m $*"; PASS=$((PASS+1)); }
bad()  { echo -e "  \033[0;31mFAIL\033[0m $*"; FAIL=$((FAIL+1)); }

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
mkdir -p "$NS/lock"; echo "$$" > "$NS/lock/pid"     # 假装有个活着的实例(测试进程自身 pid)
run_updater_args "$NR" "$NS" -- apply 0.39.0 >/dev/null 2>&1
RC=$?
rm -rf "$NS/lock"
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

# ═══════════════════════════════════════════════════════════════════
echo ""
echo "结果: PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]
