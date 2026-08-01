#!/usr/bin/env bash
# AAStar 社区节点自动更新器(MVP / increment 1)
# 设计:kms/docs/auto-update-design.md
#
# 覆盖(本地可测):
#   - TUF-lite 签名 channel manifest(minisign)+ 新鲜度(expires)+ 防回滚(metadata_version 单调 + rollback_floor)
#   - crash-safe 版本化目录 + 原子软链切换 + 状态机 + 自动回滚 + boot recovery
#   - 策略:notify-only(默认)/ security / minor / all / off;canary ring;TA 变更默认只通知
#
# 推迟(真机/CI,见设计文档 §9):RPMB TA floor、cosign/Rekor 全量、rollout 熔断、真机深度健康门。
#
# 子命令:
#   check     (默认) 拉 manifest → 校验 → 按策略应用或通知
#   recovery  boot 时调用:pending 未提交 → 自动回滚(治掉电中断)
#   status    打印当前状态
#
# 所有外部副作用都走可注入 hook(env),便于本地 mock 测试:
#   AU_RESTART_CMD / AU_HEALTH_CMD / AU_NOTIFY_CMD / AU_FETCH_CMD
set -euo pipefail

# ── 可配置(env 覆盖,默认生产路径)──────────────────────────────────
AU_ROOT="${AU_ROOT:-/opt/airaccount}"
AU_STATE_DIR="${AU_STATE_DIR:-/var/lib/airaccount/updater}"
AU_ENV_FILE="${AU_ENV_FILE:-/etc/airaccount/updater.env}"
AU_PUBKEY="${AU_PUBKEY:-/etc/airaccount/updater-pubkey.pub}"
AU_MANIFEST_BASE="${AU_MANIFEST_BASE:-https://raw.githubusercontent.com/AAStarCommunity/AirAccount/main/kms/deploy/updater/channels}"
AU_NODE_ID="${AU_NODE_ID:-$(hostname 2>/dev/null || echo unknown)}"

# hooks(测试可覆盖)
AU_RESTART_CMD="${AU_RESTART_CMD:-systemctl restart kms-api.service}"
AU_HEALTH_CMD="${AU_HEALTH_CMD:-}"          # 空 → 用内置默认健康门
AU_NOTIFY_CMD="${AU_NOTIFY_CMD:-}"          # 空 → logger/echo
AU_FETCH_CMD="${AU_FETCH_CMD:-}"            # 空 → curl(支持 file://,便于测试)

STATE_FILE="$AU_STATE_DIR/state.json"
LOCK_DIR="$AU_STATE_DIR/lock"

# ── 日志 / 通知 ──────────────────────────────────────────────────────
log()  { echo -e "\033[0;34m[updater]\033[0m $*"; }
warn() { echo -e "\033[0;33m[updater] $*\033[0m" >&2; }
die()  { echo -e "\033[0;31m[updater] $*\033[0m" >&2; notify error "$*"; exit 1; }
# 瞬态网络问题:静默退出(设计文档 §8「网络不可达→静默重试」),不发 error 告警免告警疲劳。
give_up_quiet() { warn "$*(将于下次 timer 重试)"; exit 0; }

notify() { # notify <level> <msg>
  local level="$1"; shift; local msg="$*"
  if [ -n "$AU_NOTIFY_CMD" ]; then
    $AU_NOTIFY_CMD "$level" "$msg" || true
  else
    logger -t aastar-updater "[$level] $msg" 2>/dev/null || true
    echo "[notify:$level] $msg"
  fi
}

need() { command -v "$1" >/dev/null 2>&1 || die "缺依赖 $1"; }

# ── sha256(跨 mac/linux)────────────────────────────────────────────
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

# ── semver 比较(纯 bash 3.2,x.y.z)─────────────────────────────────
ver_norm() { local v="${1#v}"; echo "${v%%-*}"; }   # 去 v 前缀 + 预发布后缀
ver_cmp() { # echo -1 / 0 / 1  表示  a<b / a==b / a>b
  local a b a1 a2 a3 b1 b2 b3
  a="$(ver_norm "$1")"; b="$(ver_norm "$2")"
  IFS=. read -r a1 a2 a3 <<EOF
$a
EOF
  IFS=. read -r b1 b2 b3 <<EOF
$b
EOF
  a1=${a1:-0}; a2=${a2:-0}; a3=${a3:-0}; b1=${b1:-0}; b2=${b2:-0}; b3=${b3:-0}
  local x y
  for pair in "$a1:$b1" "$a2:$b2" "$a3:$b3"; do
    x="${pair%%:*}"; y="${pair##*:}"
    x=$((10#${x:-0})); y=$((10#${y:-0}))
    if [ "$x" -gt "$y" ]; then echo 1; return; fi
    if [ "$x" -lt "$y" ]; then echo -1; return; fi
  done
  echo 0
}
ver_ge() { [ "$(ver_cmp "$1" "$2")" != "-1" ]; }
ver_gt() { [ "$(ver_cmp "$1" "$2")" = "1" ]; }
ver_valid() { echo "${1#v}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; }  # 严格 x.y.z
is_patch_bump() { # major==,minor==,patch>
  local a b; a="$(ver_norm "$1")"; b="$(ver_norm "$2")"
  local a1 a2 b1 b2; IFS=. read -r a1 a2 _ <<EOF
$a
EOF
  IFS=. read -r b1 b2 _ <<EOF
$b
EOF
  [ "${a1:-0}" = "${b1:-0}" ] && [ "${a2:-0}" = "${b2:-0}" ] && ver_gt "$b" "$a"
}
is_minor_or_less() { # major== , candidate>current
  local a1 b1; IFS=. read -r a1 _ _ <<EOF
$(ver_norm "$1")
EOF
  IFS=. read -r b1 _ _ <<EOF
$(ver_norm "$2")
EOF
  [ "${a1:-0}" = "${b1:-0}" ] && ver_gt "$2" "$1"
}

# ── 原子锁(mkdir,全平台)+ 统一 EXIT 清理 ─────────────────────────
WORK=""
cleanup() {
  rmdir "$LOCK_DIR" 2>/dev/null || true
  [ -n "${WORK:-}" ] && rm -rf "$WORK"
  return 0
}
acquire_lock() {
  mkdir -p "$AU_STATE_DIR"
  if ! mkdir "$LOCK_DIR" 2>/dev/null; then
    # 可能是 stale lock(上次进程被 SIGKILL/掉电,EXIT trap 没跑)。看 PID 是否还活着。
    local oldpid=""
    [ -f "$LOCK_DIR/pid" ] && oldpid="$(cat "$LOCK_DIR/pid" 2>/dev/null || true)"
    if [ -n "$oldpid" ] && kill -0 "$oldpid" 2>/dev/null; then
      warn "另一个 updater 实例在跑(pid=$oldpid),退出"; exit 0
    fi
    warn "清理 stale lock(pid=${oldpid:-?} 已不在)"
    rm -rf "$LOCK_DIR"
    mkdir "$LOCK_DIR" 2>/dev/null || { warn "抢锁失败,退出"; exit 0; }
  fi
  echo "$$" > "$LOCK_DIR/pid" 2>/dev/null || true
  trap cleanup EXIT
}

# ── 状态(原子写:tmp + mv)──────────────────────────────────────────
state_init() {
  mkdir -p "$AU_STATE_DIR"
  if [ ! -f "$STATE_FILE" ]; then
    local cur=""
    if [ -L "$AU_ROOT/current" ]; then cur="$(basename "$(readlink "$AU_ROOT/current")")"; fi
    ver_valid "$cur" || cur=""          # 非法 semver 目录名不入 state(PR#191 Low,防 ver_cmp 崩)
    jq -n --arg cur "$cur" \
      '{seen_metadata_version:0, current:$cur, previous:"", pending:""}' > "$STATE_FILE.tmp"
    mv -f "$STATE_FILE.tmp" "$STATE_FILE"
  fi
}
state_get() { jq -r --arg k "$1" '.[$k] // ""' "$STATE_FILE"; }
state_set() { # state_set k1 v1 k2 v2 ...
  local args=() ; local jqexpr="."
  while [ "$#" -ge 2 ]; do
    args+=(--arg "k_$1" "$1" --arg "v_$1" "$2")
    jqexpr="$jqexpr | .[\$k_$1]=\$v_$1"
    shift 2
  done
  jq "${args[@]}" "$jqexpr" "$STATE_FILE" > "$STATE_FILE.tmp"
  mv -f "$STATE_FILE.tmp" "$STATE_FILE"
  sync 2>/dev/null || true
}
state_set_num() { # 单独处理数字字段(metadata_version)
  jq --argjson v "$2" --arg k "$1" '.[$k]=$v' "$STATE_FILE" > "$STATE_FILE.tmp"
  mv -f "$STATE_FILE.tmp" "$STATE_FILE"; sync 2>/dev/null || true
}

# ── 下载(curl,file:// 亦可)────────────────────────────────────────
fetch() { # fetch <url> <out>
  if [ -n "$AU_FETCH_CMD" ]; then $AU_FETCH_CMD "$1" "$2"; return; fi
  curl -fSL --connect-timeout 15 --max-time 300 "$1" -o "$2"
}

# ── 验签(minisign 离线,pubkey 编入节点)──────────────────────────
verify_sig() { # verify_sig <file> <sigfile>
  need minisign
  [ -f "$AU_PUBKEY" ] || die "缺验签公钥 $AU_PUBKEY —— 拒绝任何未验签更新"
  minisign -V -p "$AU_PUBKEY" -m "$1" -x "$2" >/dev/null 2>&1
}

# ── 软链切换 ────────────────────────────────────────────────────────
# 生产板(Linux/GNU coreutils):建临时软链 + `mv -Tf` 原子 rename(2) 替换,
#   -T 不把目标当目录(避免 `mv tmp link` 跟随 link→dir 把 tmp 移进目录的坑),再 fsync 父目录。
# mac/BSD(仅测试):无 -T,回退 `ln -sfn`(非严格原子;板上不会走此分支)。
# 无论哪种,严格 crash 一致性都由 state.json 的 pending 标记 + boot recovery 兜底(见 §3.1)。
GNU_MV=0; mv --version >/dev/null 2>&1 && GNU_MV=1
swap_symlink() { # swap_symlink <linkpath> <target>
  local link="$1" target="$2" dir
  dir="$(dirname "$link")"
  if [ "$GNU_MV" = 1 ]; then
    ln -sfn "$target" "$link.tmp.$$"
    mv -Tf "$link.tmp.$$" "$link"
  else
    ln -sfn "$target" "$link"
  fi
  # fsync 父目录(GNU 有 --directory;失败则退化整机 sync)
  sync 2>/dev/null || true
}

# ── 健康门(默认内置;可 hook 覆盖)─────────────────────────────────
run_health() {
  if [ -n "$AU_HEALTH_CMD" ]; then $AU_HEALTH_CMD; return; fi
  # 内置默认:HTTP 层检查(真机深度门见设计文档 §3.2,后续接)
  local base="http://127.0.0.1:3000"
  curl -fsS --max-time 10 "$base/health" >/dev/null || return 1
  local v; v="$(curl -fsS --max-time 10 "$base/version" 2>/dev/null || echo '{}')"
  echo "$v" | jq -e '.version' >/dev/null 2>&1 || return 1
  # 核对确实是刚部署的版本:否则 restart 仍跑旧二进制(如 ExecStart drop-in 没装)
  # 会误判通过、把 state.current 记成新版而实际还是旧版(PR#191 Medium)。
  if [ -n "${AU_EXPECT_VERSION:-}" ]; then
    echo "$v" | jq -e --arg w "$AU_EXPECT_VERSION" '(.version==$w) or (.build==$w)' >/dev/null 2>&1 \
      || { warn "健康门:/version 不是期望的 $AU_EXPECT_VERSION(仍在跑旧二进制?)"; return 1; }
  fi
  return 0
}

restart_service() {
  log "restart: $AU_RESTART_CMD"
  $AU_RESTART_CMD
}

# ── vacuum 旧版本(保留 N,绝不删 current/previous)─────────────────
vacuum() {
  local keep="${1:-3}" cur prev
  cur="$(state_get current)"; prev="$(state_get previous)"
  [ -d "$AU_ROOT/releases" ] || return 0
  # 按 semver 降序列出,跳过前 keep 个,其余删(current/previous 永不删)
  local dirs count=0 name
  dirs="$(ls -1 "$AU_ROOT/releases" 2>/dev/null || true)"
  # 简单降序:用 sort -rn 无法处理 semver,退化为逐个 ver_cmp 冒泡代价高;
  # MVP 用文件 mtime 保守:只删既非 current 也非 previous、且数量超 keep 的最老者。
  # 更精确的 semver 保留留待后续。
  for name in $(ls -1t "$AU_ROOT/releases" 2>/dev/null); do
    count=$((count+1))
    if [ "$count" -le "$keep" ]; then continue; fi
    if [ "$name" = "$cur" ] || [ "$name" = "$prev" ]; then continue; fi
    log "vacuum 旧版本 $name"
    rm -rf "${AU_ROOT:?}/releases/$name"
  done
}

# ── 应用一个版本(crash-safe)────────────────────────────────────────
apply_version() { # apply_version <ver> <bundle_dir>
  local ver="$1" bundle="$2"
  local prev reldir
  prev="$(state_get current)"
  reldir="$AU_ROOT/releases/$ver"

  log "materialize releases/$ver"
  rm -rf "$reldir.new"; mkdir -p "$reldir.new"
  # bundle 里的 kms/ 内容即节点组件(TA + CA + unit + manifest)
  if [ -d "$bundle/kms" ]; then cp -R "$bundle/kms/." "$reldir.new/"; else cp -R "$bundle/." "$reldir.new/"; fi
  sync 2>/dev/null || true
  rm -rf "$reldir"; mv -f "$reldir.new" "$reldir"; sync 2>/dev/null || true

  # 标记 in-progress(先于切软链,便于 boot recovery)
  state_set pending "$ver"

  # 设 last-good 回滚目标 = prev
  if [ -n "$prev" ] && [ -d "$AU_ROOT/releases/$prev" ]; then
    swap_symlink "$AU_ROOT/last-good" "releases/$prev"
  fi

  # 切 current(生产还需切 TA 固定路径,见设计文档 §3.1;此处 hook 化留待真机)
  swap_symlink "$AU_ROOT/current" "releases/$ver"

  restart_service || { warn "restart 失败,回滚"; rollback "$prev"; return 1; }

  log "健康门检查…"
  export AU_EXPECT_VERSION="$(ver_norm "$ver")"   # 供内置/外部健康门核对部署后版本
  if run_health; then
    state_set previous "$prev" current "$ver" pending ""
    notify info "更新成功 $prev → $ver(node=$AU_NODE_ID)"
    vacuum "${KEEP_RELEASES:-3}"
    return 0
  else
    warn "健康门未过,回滚到 $prev"
    rollback "$prev"
    return 1
  fi
}

rollback() { # rollback <target_ver>
  local target="$1"
  if [ -z "$target" ] || [ ! -d "$AU_ROOT/releases/$target" ]; then
    if [ -L "$AU_ROOT/last-good" ]; then
      swap_symlink "$AU_ROOT/current" "$(readlink "$AU_ROOT/last-good")"
      target="$(basename "$(readlink "$AU_ROOT/last-good")")"
    else
      state_set pending ""
      notify error "回滚失败:无可用 last-good(node=$AU_NODE_ID)—— 转 OOB 人工救板"
      return 1
    fi
  else
    swap_symlink "$AU_ROOT/current" "releases/$target"
  fi
  restart_service || warn "回滚后 restart 也失败"
  state_set current "$target" pending ""
  notify warn "已回滚到 $target(node=$AU_NODE_ID)"
}

# ── boot recovery:pending 未提交 → 回滚 ────────────────────────────
cmd_recovery() {
  # boot 早期单线程:清 stale lock,别因它阻塞恢复(最需要恢复的正是掉电场景)。
  rm -rf "$LOCK_DIR" 2>/dev/null || true
  # recovery 不同步 restart:本 unit Before=kms-api,restart 会与 boot 事务自锁。
  # 只切回 symlink + state,kms-api 由正常 boot 顺序启动(读到已回滚的 current)。
  AU_RESTART_CMD="true"
  state_init
  local pending; pending="$(state_get pending)"
  if [ -z "$pending" ]; then log "无 pending,无需恢复"; return 0; fi
  warn "检测到未提交更新 pending=$pending(疑似掉电中断)→ 回滚"
  # 回滚目标 = state.current:它在提交前始终是「上次已提交的版本」,才是正确的已知良好版。
  local target; target="$(state_get current)"
  rollback "$target"
  notify warn "boot recovery:掉电中断已回滚到 $target(node=$AU_NODE_ID)"
}

# ── status ──────────────────────────────────────────────────────────
cmd_status() { state_init; cat "$STATE_FILE"; }

# ── check:主流程 ───────────────────────────────────────────────────
cmd_check() {
  need jq; need curl
  acquire_lock
  state_init
  # 载入策略 env
  AUTO_UPDATE="notify-only"; CHANNEL="stable"; UPDATE_POLICY="security"
  TA_AUTO_UPDATE="off"; PIN_VERSION=""; KEEP_RELEASES=3
  # shellcheck disable=SC1090
  [ -f "$AU_ENV_FILE" ] && . "$AU_ENV_FILE"

  WORK="$(mktemp -d)"

  # 1) 拉 signed manifest + sig
  local murl="$AU_MANIFEST_BASE/$CHANNEL.json"
  log "拉 manifest: $murl"
  # manifest 本体拉不到 = 网络/GitHub 不可达 → 静默重试(设计文档 §8)
  fetch "$murl" "$WORK/channel.json" || give_up_quiet "manifest 下载失败(网络?)"
  # 但 manifest 拉到了、签名却拉不到 = 异常(发布出错/篡改)→ fail-closed 告警,不能静默
  fetch "$murl.minisig" "$WORK/channel.json.minisig" || die "manifest 有正文却无签名 —— 拒绝(fail-closed)"

  # 2) 验签(不过绝不继续)
  verify_sig "$WORK/channel.json" "$WORK/channel.json.minisig" || \
    die "manifest 验签失败 —— 疑似投毒/损坏,拒绝"

  # 2.5) schema 校验(fail-closed):类型/必填/semver/sha256 格式。
  #      放在持久化 seen_metadata_version 之前 —— 否则坏 schema 的高版本会污染 seen,
  #      之后正确的较低修正版会被防回滚误拒。
  jq -e '
    (.metadata_version|type=="number" and .==floor)
    and (.expires|type=="string" and test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))
    and (.rollback_floor==null or (.rollback_floor|type=="string" and test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$")))
    and (.releases|type=="array")
    and (all(.releases[];
          (.version|type=="string" and test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$"))
          and (.tarball|type=="string" and (length>0))
          and (.sha256|type=="string" and test("^[0-9a-fA-F]{64}$"))
          and ((.security//false)|type=="boolean")
          and ((.auto_apply_allowed//false)|type=="boolean")
          and ((.ta_changed//false)|type=="boolean")
          and ((.min_version//"0.0.0")|type=="string" and test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$"))
          and (.requires_ta_version==null or (.requires_ta_version|type=="string"))
          and (.canary_ring==null or ((.canary_ring|type=="array") and (all(.canary_ring[];type=="string"))))))
  ' "$WORK/channel.json" >/dev/null 2>&1 || die "manifest schema 非法 —— 拒绝(fail-closed)"

  # 3) 新鲜度(expires 必填)—— 防 freeze attack
  local expires now exp_epoch
  expires="$(jq -r '.expires' "$WORK/channel.json")"
  now="$(date -u +%s)"
  exp_epoch="$(date -u -d "$expires" +%s 2>/dev/null || date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$expires" +%s 2>/dev/null || echo 0)"
  [ "$exp_epoch" -gt 0 ] || die "manifest expires 无法解析($expires)—— 拒绝(fail-closed)"
  [ "$now" -gt "$exp_epoch" ] && die "manifest 已过期($expires)—— 疑似 freeze 攻击或长期离线,拒绝"

  # 4) 防回滚:metadata_version 必须单调不降(此时已过 schema 校验)
  local mver seen
  mver="$(jq -r '.metadata_version' "$WORK/channel.json")"
  seen="$(state_get seen_metadata_version)"; seen="${seen:-0}"
  if [ "$mver" -lt "$seen" ]; then
    die "manifest metadata_version($mver) < 已见($seen)—— 回滚攻击,拒绝"
  fi
  [ "$mver" -gt "$seen" ] && state_set_num seen_metadata_version "$mver"

  # 5) rollback_floor
  local floor cur
  floor="$(jq -r '.rollback_floor // "0.0.0"' "$WORK/channel.json")"
  cur="$(state_get current)"; [ -z "$cur" ] && cur="0.0.0"
  # state.current 来自 installer 建的 current 软链名,从未校验;非法 semver 会让 ver_cmp
  # 的算术在 set -e 下崩脚本(PR#191 Low)。fail-safe 归一到 0.0.0。
  ver_valid "$cur" || { warn "state.current '$cur' 非法 semver → 按 0.0.0 处理"; cur="0.0.0"; }

  # 6) 遍历所有候选(version>cur, >=floor, min_version 满足),逐个按策略判定:
  #    - apply 目标 = 能被 apply 的「最高」版本(不是版本最高的那个)—— 否则一个更高的
  #      非安全 minor 会挡住本该自动打的安全 patch,架空 security 策略(PR#191 High-1)。
  #    - notify 目标 = 所有候选里的最高版本(仅告知)。
  # 用 US(\037,非空白)作分隔符:tab 是 IFS 空白会折叠空字段,导致列错位。
  local US=$'\037'
  local ac_ver="" ac_tarball="" ac_sha=""      # apply 候选(能自动应用的最高版)
  local nc_ver=""                               # notify 候选(最高版,仅告知)
  local r_ver r_sec r_auto r_ta r_min r_reqta r_tarball r_sha r_canary act
  while IFS="$US" read -r r_ver r_sec r_auto r_ta r_min r_reqta r_tarball r_sha r_canary; do
    [ -z "$r_ver" ] && continue
    ver_gt "$r_ver" "$cur" || continue
    ver_ge "$r_ver" "$floor" || continue
    ver_ge "$cur" "$r_min" || continue           # 上升路径:当前须 >= 该版本要求的 min
    if [ -z "$nc_ver" ] || ver_gt "$r_ver" "$nc_ver"; then nc_ver="$r_ver"; fi
    act="$(decide_action "$cur" "$r_ver" "$r_sec" "$r_auto" "$r_ta" "$r_canary" "$r_reqta")"
    if [ "$act" = "apply" ] && { [ -z "$ac_ver" ] || ver_gt "$r_ver" "$ac_ver"; }; then
      ac_ver="$r_ver"; ac_tarball="$r_tarball"; ac_sha="$r_sha"
    fi
  done <<EOF
$(jq -r '.releases[] | [(.version),(.security//false|tostring),(.auto_apply_allowed//false|tostring),(.ta_changed//false|tostring),(.min_version//"0.0.0"),(.requires_ta_version//""),(.tarball//""),(.sha256//""),((.canary_ring//[])|join(","))] | join("\u001f")' "$WORK/channel.json")
EOF

  # rollback_floor 越界告警(当前版本已低于 floor)
  if ! ver_ge "$cur" "$floor"; then
    notify warn "当前版本 $cur 低于 rollback_floor $floor(存在已知漏洞)—— 应尽快升级(node=$AU_NODE_ID)"
  fi

  if [ -z "$nc_ver" ]; then log "无更高候选版本(current=$cur)"; return 0; fi

  # 7) 无可自动应用的候选 → 只通知最高版
  if [ -z "$ac_ver" ]; then
    notify info "有新版本可用:$cur → ${nc_ver}。当前策略未自动应用,请人工确认(node=$AU_NODE_ID)"
    return 0
  fi
  log "apply 目标 $ac_ver(候选最高 $nc_ver,current=$cur)"
  # 若存在比 apply 目标更高、但不自动应用的版本,也告知一声
  if ver_gt "$nc_ver" "$ac_ver"; then
    notify info "另有更高版本 $nc_ver 可用(未自动应用,需人工确认;node=$AU_NODE_ID)"
  fi

  # 8) 下载 apply 目标 tarball + 校验 sha256(+ 可选独立 tarball 签名)
  [ -n "$ac_tarball" ] || die "apply 候选缺 tarball URL"
  log "下载 $ac_tarball"
  fetch "$ac_tarball" "$WORK/node.tgz" || give_up_quiet "tarball 下载失败(网络?)"
  local got; got="$(sha256_of "$WORK/node.tgz")"
  [ "$got" = "$ac_sha" ] || die "tarball sha256 不匹配(期望 $ac_sha 得 $got)—— 拒绝"
  # tarball 独立 minisig(可选,存在则必须过)
  if fetch "$ac_tarball.minisig" "$WORK/node.tgz.minisig" 2>/dev/null; then
    verify_sig "$WORK/node.tgz" "$WORK/node.tgz.minisig" || die "tarball 验签失败 —— 拒绝"
  fi

  # tar 路径加固:拒绝绝对路径 / .. / 非单一 airaccount-node-* 顶层(防越界写)
  local entries; entries="$(tar -tzf "$WORK/node.tgz")"
  echo "$entries" | grep -qE '(^/|(^|/)\.\.(/|$))' && die "tarball 含绝对路径或 ..  —— 拒绝"
  # 拒绝特殊文件条目:symlink/hardlink 挡 link 越界,char/block/fifo/socket 防 root 建设备节点
  # (-tv 首字符 l=symlink h=hardlink c=字符设备 b=块设备 p=FIFO s=socket)。bundle 全是普通文件。
  tar -tvzf "$WORK/node.tgz" | awk '{print substr($1,1,1)}' | grep -qE '^[lhbcps]$' \
    && die "tarball 含 symlink/hardlink/设备/FIFO 条目 —— 拒绝"
  local tops; tops="$(echo "$entries" | sed 's#/.*##' | sort -u)"
  [ "$(echo "$tops" | wc -l | tr -d ' ')" = "1" ] || die "tarball 顶层非单一目录 —— 拒绝"
  echo "$tops" | grep -qE '^airaccount-node-' || die "tarball 顶层非 airaccount-node-* —— 拒绝"
  mkdir -p "$WORK/x"; tar -xzf "$WORK/node.tgz" -C "$WORK/x"
  local bundle; bundle="$(find "$WORK/x" -maxdepth 1 -type d -name 'airaccount-node-*' 2>/dev/null | head -1)"
  [ -z "$bundle" ] && bundle="$WORK/x"

  # 9) 应用(crash-safe + 健康门 + 回滚)
  mkdir -p "$AU_ROOT/releases"
  apply_version "$ac_ver" "$bundle"
}

decide_action() { # decide_action cur cand security auto_apply ta_changed canary requires_ta_version → apply|notify|skip
  local cur="$1" cand="$2" sec="$3" auto="$4" ta="$5" canary="$6" reqta="$7"
  # 总开关优先(posture 先于一切,含 PIN)—— PR#191 High-2:
  # 运维只想「锁死版本」而设 PIN 但保留默认 notify-only,不能被 PIN 绕过自动应用。
  [ "$AUTO_UPDATE" = "on" ] || { echo notify; return; }
  [ "$UPDATE_POLICY" = "off" ] && { echo notify; return; }
  # PIN:白名单锁定。非 PIN 版本一律 skip(锁死);PIN 版本继续走下面的安全门。
  if [ -n "$PIN_VERSION" ] && [ "$PIN_VERSION" != "$cand" ]; then echo skip; return; fi
  # TA 变更且未开 TA 自动 → 只通知(设计文档 §3.2)
  if [ "$ta" = "true" ] && [ "$TA_AUTO_UPDATE" != "on" ]; then echo notify; return; fi
  # 兼容性门(PR#191 Medium):CA 要求的 TA 比当前新,但本次不换 TA(ta_changed=false)→
  # 无法满足 → 只通知,不自动升到与旧 TA 不兼容的 CA。
  if [ -n "$reqta" ] && [ "$ta" != "true" ] && ver_gt "$reqta" "$cur"; then echo notify; return; fi
  # canary:release 指定了 ring 时,仅 ring 内节点自动
  if [ -n "$canary" ]; then
    case ",$canary," in *,"$AU_NODE_ID",*) : ;; *) echo notify; return;; esac
  fi
  # PIN 版本匹配且已过上面所有安全门 → 显式应用(运维明确指定,不再受 security/minor 策略约束)
  if [ -n "$PIN_VERSION" ]; then echo apply; return; fi
  case "$UPDATE_POLICY" in
    all)      echo apply ;;
    minor)    if is_minor_or_less "$cur" "$cand"; then echo apply; else echo notify; fi ;;
    security)
      if [ "$sec" = "true" ] && [ "$auto" = "true" ] && is_patch_bump "$cur" "$cand"; then
        echo apply
      else echo notify; fi ;;
    *) echo notify ;;
  esac
}

main() {
  case "${1:-check}" in
    check)    cmd_check ;;
    recovery) cmd_recovery ;;
    status)   cmd_status ;;
    *) die "用法: $0 {check|recovery|status}" ;;
  esac
}
main "$@"
