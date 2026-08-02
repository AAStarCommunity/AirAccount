#!/usr/bin/env bash
# 本地测试:社区节点自动更新器(无需硬件)。
# 覆盖设计文档 §11 的 6 条路径:
#   T1 升级成功(security patch)   T2 验签失败拒绝     T3 过期 manifest 拒绝(freeze)
#   T4 回滚攻击拒绝(metadata_version 下降)  T5 健康门失败→回滚   T6 掉电→boot recovery
# 附加:T7 notify-only 不自动应用   T8 TA 变更 + TA_AUTO_UPDATE=off 只通知
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
echo "== T20 manifest 端点 HTTP 4xx(curl 22)→ 告警拒绝(不静默)=="
read NR NS < <(new_node t20 0.28.0)
printf '#!/usr/bin/env bash\nexit 22\n' > "$ROOT/fetch-4xx.sh"; chmod +x "$ROOT/fetch-4xx.sh"
if env AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$ROOT/updater.env" AU_PUBKEY="$ROOT/pub.key" \
      AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=testnode \
      AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
      AU_FETCH_CMD="$ROOT/fetch-4xx.sh" bash "$UPDATER" check >/dev/null 2>&1; then
  bad "HTTP 4xx 却成功了(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "4xx 拒绝,current 未变" || bad "current=$(st "$NS" current)"
fi

# ═══════════════════════════════════════════════════════════════════
echo "== T20b manifest 网络失败(curl 7)→ 静默退出(exit 0),不告警 =="
read NR NS < <(new_node t20b 0.28.0)
printf '#!/usr/bin/env bash\nexit 7\n' > "$ROOT/fetch-net.sh"; chmod +x "$ROOT/fetch-net.sh"
env AU_ROOT="$NR" AU_STATE_DIR="$NS" AU_ENV_FILE="$ROOT/updater.env" AU_PUBKEY="$ROOT/pub.key" \
    AU_MANIFEST_BASE="file://$SERVER/channels" AU_NODE_ID=testnode \
    AU_RESTART_CMD="$ROOT/mock-restart.sh" AU_HEALTH_CMD="$ROOT/mock-health.sh" AU_NOTIFY_CMD="$ROOT/mock-notify.sh" \
    AU_FETCH_CMD="$ROOT/fetch-net.sh" bash "$UPDATER" check >/dev/null 2>&1
RC=$?
[ "$RC" = 0 ] && ok "网络失败静默退出(exit 0)" || bad "exit=$RC(应 0)"

# ═══════════════════════════════════════════════════════════════════
echo "== T21 requires_ta_version 非 semver → schema fail-closed =="
read NR NS < <(new_node t21 0.28.0)
S="$(make_bundle 0.29.0 TA-0.28.0)"
REL="$(jq -n --arg s "$S" --arg u "file://$SERVER/airaccount-node-0.29.0.tar.gz" \
  '[{version:"0.29.0",security:true,auto_apply_allowed:true,ta_changed:false,requires_ta_version:"latest",min_version:"0.0.0",tarball:$u,sha256:$s}]')"
write_manifest 21 "2035-01-01T00:00:00Z" "0.0.0" "$REL"
if run_updater "$NR" "$NS" check >/dev/null 2>&1; then
  bad "非法 requires_ta_version 却成功(应拒绝)"
else
  [ "$(st "$NS" current)" = "0.28.0" ] && ok "非法 requires_ta_version,current 未变" || bad "current=$(st "$NS" current)"
  [ "$(st "$NS" seen_metadata_version)" = "0" ] && ok "seen 未污染(仍 0)" || bad "seen=$(st "$NS" seen_metadata_version)"
fi

# ═══════════════════════════════════════════════════════════════════
echo ""
echo "结果: PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]
