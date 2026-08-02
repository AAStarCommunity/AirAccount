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
LOCK_LINK="$AU_STATE_DIR/lock"   # 互斥锁 = 原子 symlink,target 为持有者 pid(见 acquire_lock)

# ── 日志 / 通知 ──────────────────────────────────────────────────────
log()  { echo -e "\033[0;34m[updater]\033[0m $*"; }
warn() { echo -e "\033[0;33m[updater] $*\033[0m" >&2; }
die()  { echo -e "\033[0;31m[updater] $*\033[0m" >&2; notify error "$*"; exit 1; }
# 瞬态网络问题:静默退出(设计文档 §8「网络不可达→静默重试」),不发 error 告警免告警疲劳。
give_up_quiet() { warn "$*(将于下次 timer 重试)"; exit 0; }
# 拉取关键文件并按失败类型分流(合并 #192 的 fetch_required + 本 PR 的 STRICT_FETCH):
#   curl 22 = HTTP 4xx/5xx(端点响应了但报错)= 配置错/撤除/篡改 → 一律 die(不静默,#192)。
#   其它(DNS/连接/超时等网络类):`apply`(STRICT_FETCH=1,运维显式指定)硬失败,否则
#   `check`(定时器)静默重试(设计文档 §8,免告警疲劳)。
fetch_required() { # fetch_required <url> <out> <what>
  local rc=0
  fetch "$1" "$2" || rc=$?
  [ "$rc" = 0 ] && return 0
  [ "$rc" = 22 ] && die "$3:端点返回 HTTP 错误(curl 22)—— 端点配置/撤除/篡改?拒绝"
  if [ "${STRICT_FETCH:-0}" = 1 ]; then die "$3:下载失败(网络?rc=$rc)—— apply 不静默"; fi
  give_up_quiet "$3:下载失败(网络?rc=$rc)"
}

notify() { # notify <level> <msg> —— set -e 安全:永远返回 0(通知失败不阻断主流程)
  notify_send "$@" || true
}
# 真实送达状态版(供去重判定,codex High-3):返回 hook/内置通道的真实退出码。
# 约定:hook 送达成功=0;送达失败(网络等)=非 0;凭据未配置(无处可送)=0(视为无需重推)。
notify_send() { # notify_send <level> <msg>
  local level="$1"; shift; local msg="$*"
  if [ -n "$AU_NOTIFY_CMD" ]; then
    $AU_NOTIFY_CMD "$level" "$msg"
  else
    logger -t aastar-updater "[$level] $msg" 2>/dev/null || true
    echo "[notify:$level] $msg"
  fi
}

# ── 持久化告警队列(pr-daemon 三轮 #3)──────────────────────────────
# boot-recovery unit 跑在 network-online 之前,notify-telegram 一次性 curl 必然失败、
# 且 notify() 的 `|| true` 吞掉 → 「板子砖了已回滚」这条最重要的告警送不出去。
# 解法:recovery 立即发一次(网络若已起就成),失败则**入队**;下次 check(定时器,网络已起)
# 开头 flush 投递。队列文件在状态目录(非 release 树),首行 level 余下正文。
queue_notify() { # <level> <msg>
  mkdir -p "$AU_STATE_DIR" 2>/dev/null || true
  { printf '%s\n' "$1"; printf '%s' "$2"; } > "$AU_STATE_DIR/pending-notify.tmp" 2>/dev/null \
    && mv -f "$AU_STATE_DIR/pending-notify.tmp" "$AU_STATE_DIR/pending-notify" 2>/dev/null || true
  sync 2>/dev/null || true   # 这正是掉电恢复路径,紧接着可能再掉电 → fsync 保证告警落盘(pr-daemon 四轮 Low)
}
flush_pending_notify() {
  local f="$AU_STATE_DIR/pending-notify"
  [ -f "$f" ] || return 0
  local lvl msg
  lvl="$(head -1 "$f" 2>/dev/null | tr -d '[:space:]')"
  # level 白名单:空/坏首行不能直接喂给通知 hook(pr-daemon 四轮 Low)。
  case "$lvl" in info|warn|error) : ;; *) lvl=warn ;; esac
  msg="$(tail -n +2 "$f" 2>/dev/null || true)"
  if notify_send "$lvl" "$msg"; then rm -f "$f" 2>/dev/null || true; log "已投递挂起告警"; else
    warn "挂起告警仍投递失败,下次 check 再试"; fi
}

# 载入策略默认 + source updater.env(set -a 导出 TELEGRAM_* 给通知 hook 子进程)。
# 必须**尽早**调用(每个子命令入口),这样此后任何 die 的告警都能带上 Telegram 凭据送达
# —— 否则早期失败(缺依赖/锁/损坏)恰恰最该告警,却因 env 还没 source 而静默(pr-daemon #2)。
# cmd_recovery 也调它,否则「掉电回滚」这种最关键告警结构性发不出去。
load_policy_env() {
  AUTO_UPDATE="notify-only"; CHANNEL="stable"; UPDATE_POLICY="security"
  TA_AUTO_UPDATE="off"; PIN_VERSION=""; KEEP_RELEASES=3
  # shellcheck disable=SC1090
  [ -f "$AU_ENV_FILE" ] && { set -a; . "$AU_ENV_FILE"; set +a; }
  return 0   # 关键(pr-daemon 四轮 #1):无 env 文件时 `[ -f ]&&…` 返回 1,而本函数是 main 第一句,
             # set -e 下会让所有子命令(含 status/recovery)在跑之前静默 exit 1。显式 return 0。
}

need() { command -v "$1" >/dev/null 2>&1 || die "缺依赖 $1"; }

# ── sha256(跨 mac/linux)────────────────────────────────────────────
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

# 真实 TA 版本(兼容门不能用 CA 版本近似 = fail-open)。来源**必须在 release 树之外**
# (pr-daemon 三轮 #1):releases/<ver>/ 是从内容不可信的 tarball 解出的、且 rollback 会
# 重指 current —— 若把 TA_VERSION 放那里:①current 一重指就消失 → 又 fail-close;
# ②CA-only bundle 能自带 TA_VERSION 自证从未刷过的 TA → 重新 fail-open。
# 故只认:env AU_TA_VERSION(运维声明) > $AU_STATE_DIR/ta-version(installer/OOB 写,状态目录,
# 不在 release 树、不随 rollback 变)。拿不到 → 回空;调用方一律 fail-closed。
ta_version() {
  if [ -n "${AU_TA_VERSION:-}" ]; then echo "${AU_TA_VERSION#v}" | tr -d '[:space:]'; return; fi
  # 固定读状态目录(不给 AU_TA_VERSION_FILE 覆盖 —— 否则 updater.env 被 set -a source,
  # 运维可把它指回 $AU_ROOT/ 下,重新引入 release 树耦合,pr-daemon 四轮 Low)。
  local f="$AU_STATE_DIR/ta-version"
  [ -r "$f" ] && { head -1 "$f" 2>/dev/null | tr -d '[:space:]v'; return; }
  echo ""
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

# ── 互斥锁:原子 symlink(全平台,无 flock 依赖)+ 统一 EXIT 清理 ────────
# 锁 = 一个 symlink,target 就是持有者 pid。`ln -s "$$" LOCK` 是**单个原子 syscall**
# 且 name 已存在即失败 —— 没有「创建后再单独写 pid」的两步,故没有 mkdir/pid 之间的
# TOCTOU 窗口,并发也绝不会两个进程都以为持锁(pr-daemon 三轮 #4 复现的 race)。
# macOS 无 flock(1),故不用 flock;symlink 原子性 POSIX 保证,mac/Linux 一致。
WORK=""
cleanup() {
  # 只删自己持有的锁(symlink target==$$);trap 在成功持锁后才设,故到这里必是我们持有。
  [ "$(readlink "$LOCK_LINK" 2>/dev/null || true)" = "$$" ] && rm -f "$LOCK_LINK" 2>/dev/null || true
  [ -n "${WORK:-}" ] && rm -rf "$WORK"
  return 0
}
# 锁竞争统一退出:apply(LOCK_FATAL=1)硬失败;check 静默退 0 等下轮。
lock_contended() { # <msg>
  if [ "${LOCK_FATAL:-0}" = 1 ]; then die "$1 —— apply 中止(未做任何更改)"; fi
  warn "$1,退出"; exit 0
}
acquire_lock() {
  mkdir -p "$AU_STATE_DIR"
  local tries=0 oldpid
  while ! ln -s "$$" "$LOCK_LINK" 2>/dev/null; do   # 原子:name 已存在即失败
    oldpid="$(readlink "$LOCK_LINK" 2>/dev/null || true)"
    if [ -n "$oldpid" ] && kill -0 "$oldpid" 2>/dev/null; then
      lock_contended "另一个 updater 实例在跑(pid=$oldpid)"   # 持有者活着 → 竞争(绝不清活锁)
    fi
    # 持有者已死(或链接损坏)→ 清掉重试。ln 原子:并发 reclaim 也只有一个 ln 成功,
    # 落败者下轮看到活锁 → contended。故不会两个进程都持锁。
    rm -f "$LOCK_LINK" 2>/dev/null || true
    tries=$((tries+1)); [ "$tries" -ge 5 ] && lock_contended "抢锁反复失败(竞争?)"
  done
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
    # last-good 必须是「指向真实存在目录」的软链 —— 只判 -L(是软链)不够:若 last-good
    # 悬空(目标 release 已被删,掉电+FS 损坏时正会如此),会把 current 指向不存在的版本、
    # 谎报回滚成功而板上零个有效 release(pr-daemon 四轮 #4,已复现)。加 -d(带尾 / 强制解析软链)。
    if [ -L "$AU_ROOT/last-good" ] && [ -d "$AU_ROOT/last-good/" ]; then
      swap_symlink "$AU_ROOT/current" "$(readlink "$AU_ROOT/last-good")"
      target="$(basename "$(readlink "$AU_ROOT/last-good")")"
    else
      state_set pending ""
      # 板子实质不可恢复 —— 这是全系统最该送达的告警。立即试,失败则入队(recovery 无网窗口),
      # 下次 check flush(pr-daemon 四轮 #3:原来 notify||true fire-and-forget,此告警永久丢失)。
      local emsg="回滚失败:无可用 last-good(node=$AU_NODE_ID)—— 板子不可恢复,转 OOB 人工救板"
      notify_send error "$emsg" || queue_notify error "$emsg"
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
  # env 已由 main 最先 source(含 Telegram 凭据)。
  # boot 早期单线程:清可能残留的 stale 锁软链(崩溃进程的软链不会自动消失),别阻塞恢复。
  rm -f "$LOCK_LINK" 2>/dev/null || true
  # recovery 不同步 restart:本 unit Before=kms-api,restart 会与 boot 事务自锁。
  # 只切回 symlink + state,kms-api 由正常 boot 顺序启动(读到已回滚的 current)。
  AU_RESTART_CMD="true"
  state_init
  local pending; pending="$(state_get pending)"
  if [ -z "$pending" ]; then log "无 pending,无需恢复"; return 0; fi
  warn "检测到未提交更新 pending=$pending(疑似掉电中断)→ 回滚"
  # 回滚目标 = state.current:它在提交前始终是「上次已提交的版本」,才是正确的已知良好版。
  local target; target="$(state_get current)"
  # `|| true`:rollback 走「无 last-good 不可恢复」分支时 return 1,而它已自行发/入队了那条
  # 更紧急的告警;这里不能因 set -e 提前中断(否则连成功回滚的告警也发不出,pr-daemon 四轮 #3)。
  if rollback "$target"; then
    # 这条(回滚成功)也在 recovery 无网窗口 → 立即试一次,失败入队,下次 check flush。
    local rmsg="boot recovery:掉电中断已回滚到 $target(node=$AU_NODE_ID)"
    notify_send warn "$rmsg" || { queue_notify warn "$rmsg"; warn "告警已入队,待下次 check 投递"; }
  fi
}

# ── status ──────────────────────────────────────────────────────────
cmd_status() { state_init; cat "$STATE_FILE"; }

# ── 拉取 + 验签 + schema + 新鲜度 + 防回滚:成功后 $WORK/channel.json 可信 ──
# check 与 apply 共用(单一可信实现)。设全局 MANIFEST / FLOOR / CUR / MVER。
load_manifest() {
  local murl="$AU_MANIFEST_BASE/$CHANNEL.json"
  log "拉 manifest: $murl"
  # manifest 拉取:curl 22(4xx/5xx)→ die;网络类 → check 静默重试 / apply 硬失败
  fetch_required "$murl" "$WORK/channel.json" "manifest"
  # 拉到正文却无签名 = 异常(发布出错/篡改)→ fail-closed 告警
  fetch "$murl.minisig" "$WORK/channel.json.minisig" || die "manifest 有正文却无签名 —— 拒绝(fail-closed)"
  # 验签(不过绝不继续)
  verify_sig "$WORK/channel.json" "$WORK/channel.json.minisig" || \
    die "manifest 验签失败 —— 疑似投毒/损坏,拒绝"
  # schema 校验(fail-closed)—— 放在持久化 seen_metadata_version 之前
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
          and ((.severity//"none")|type=="string" and test("^(none|low|medium|high|critical)$"))
          and (.notes==null or (.notes|type=="string" and (test("[[:cntrl:]]")|not) and (length<=280)))
          and ((.min_version//"0.0.0")|type=="string" and test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$"))
          and (.requires_ta_version==null or (.requires_ta_version|type=="string" and test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$")))
          and (.canary_ring==null or ((.canary_ring|type=="array") and (all(.canary_ring[];type=="string"))))))
  ' "$WORK/channel.json" >/dev/null 2>&1 || die "manifest schema 非法 —— 拒绝(fail-closed)"
  # 新鲜度(expires 必填)—— 防 freeze attack
  local expires now exp_epoch
  expires="$(jq -r '.expires' "$WORK/channel.json")"
  now="$(date -u +%s)"
  exp_epoch="$(date -u -d "$expires" +%s 2>/dev/null || date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$expires" +%s 2>/dev/null || echo 0)"
  [ "$exp_epoch" -gt 0 ] || die "manifest expires 无法解析($expires)—— 拒绝(fail-closed)"
  [ "$now" -gt "$exp_epoch" ] && die "manifest 已过期($expires)—— 疑似 freeze 攻击或长期离线,拒绝"
  # 防回滚:metadata_version 单调不降
  local mver seen
  mver="$(jq -r '.metadata_version' "$WORK/channel.json")"
  seen="$(state_get seen_metadata_version)"; seen="${seen:-0}"
  if [ "$mver" -lt "$seen" ]; then
    die "manifest metadata_version($mver) < 已见($seen)—— 回滚攻击,拒绝"
  fi
  [ "$mver" -gt "$seen" ] && state_set_num seen_metadata_version "$mver"
  # rollback_floor + 当前版本(fail-safe 归一非法 semver 到 0.0.0)
  FLOOR="$(jq -r '.rollback_floor // "0.0.0"' "$WORK/channel.json")"
  CUR="$(state_get current)"; [ -z "$CUR" ] && CUR="0.0.0"
  ver_valid "$CUR" || { warn "state.current '$CUR' 非法 semver → 按 0.0.0 处理"; CUR="0.0.0"; }
  MANIFEST="$WORK/channel.json"; MVER="$mver"
}

# ── 下载 tarball + sha256 + 可选独立验签 + tar 加固 + 解压 → apply_version ──
# check 与 apply 共用。tarball 的 hash/URL 必须由调用方从「已验签 manifest」取。
download_verify_apply() { # <ver> <tarball> <sha>
  local ac_ver="$1" ac_tarball="$2" ac_sha="$3"
  [ -n "$ac_tarball" ] || die "候选缺 tarball URL"
  log "下载 $ac_tarball"
  fetch_required "$ac_tarball" "$WORK/node.tgz" "tarball"
  # sha256 大小写不敏感比较(pr-daemon #4):schema 的 [0-9a-fA-F] 允许大写,而 sha256_of
  # 输出小写 → 大写 manifest 会永久 fail-closed 拒绝合法更新。两边归一到小写再比。
  local got exp
  got="$(sha256_of "$WORK/node.tgz" | tr 'A-F' 'a-f')"
  exp="$(printf '%s' "$ac_sha" | tr 'A-F' 'a-f')"
  [ "$got" = "$exp" ] || die "tarball sha256 不匹配(期望 $ac_sha 得 $got)—— 拒绝"
  # tarball 独立 minisig(可选,存在则必须过)
  if fetch "$ac_tarball.minisig" "$WORK/node.tgz.minisig" 2>/dev/null; then
    verify_sig "$WORK/node.tgz" "$WORK/node.tgz.minisig" || die "tarball 验签失败 —— 拒绝"
  fi
  # tar 路径加固:拒绝绝对路径 / .. / 特殊文件 / 非单一 airaccount-node-* 顶层。
  # 注(codex High-2 复核):所有匹配都用 `grep`(**不带 -q**)—— grep -q 命中即早退会让
  # 上游 tar/awk/echo 收 SIGPIPE,在 pipefail 下把整条 pipeline 翻成非零,使 `&& die` 不执行,
  # 恶意条目反被放过。去掉 -q 让 grep 读完全部输入,消除该早退旁路。
  local entries; entries="$(tar -tzf "$WORK/node.tgz")" || die "tarball 无法读取(损坏?)—— 拒绝"
  echo "$entries" | grep -E '(^/|(^|/)\.\.(/|$))' >/dev/null && die "tarball 含绝对路径或 ..  —— 拒绝"
  local ftypes; ftypes="$(tar -tvzf "$WORK/node.tgz" | awk '{print substr($1,1,1)}')" \
    || die "tarball 无法读取(损坏?)—— 拒绝"
  echo "$ftypes" | grep -E '^[lhbcps]$' >/dev/null && die "tarball 含 symlink/hardlink/设备/FIFO 条目 —— 拒绝"
  local tops; tops="$(echo "$entries" | sed 's#/.*##' | sort -u)"
  [ "$(echo "$tops" | wc -l | tr -d ' ')" = "1" ] || die "tarball 顶层非单一目录 —— 拒绝"
  echo "$tops" | grep -E '^airaccount-node-' >/dev/null || die "tarball 顶层非 airaccount-node-* —— 拒绝"
  mkdir -p "$WORK/x"; tar -xzf "$WORK/node.tgz" -C "$WORK/x"
  local bundle; bundle="$(find "$WORK/x" -maxdepth 1 -type d -name 'airaccount-node-*' 2>/dev/null | head -1)"
  [ -z "$bundle" ] && bundle="$WORK/x"

  # TA 实际内容门(codex High-2):不信 manifest 的 ta_changed 一面之词,校验 bundle
  # 里实际的 TA 是否真变了。未授权 TA 变更(TA_OK!=1)时,枚举 bundle 与当前 current 下
  # **所有** *.ta 的「文件名:sha256」有序集合(codex 复核 High-1:不能只看第一个 .ta,
  # 否则 bundle 混入多个 TA 时变化的那个可能被漏过),任何增/删/改即算 TA 变更 → 拒绝。
  # 仅当有基线 TA 可比对时强制;flat/首装无基线时告警跳过(TA 版本化未完,设计 §9)。
  if [ "${TA_OK:-0}" != 1 ]; then
    local bundle_ta cur_ta="" cur_dir=""
    bundle_ta="$(ta_set "$bundle")"
    if [ -n "$bundle_ta" ]; then                     # bundle 带了 TA 才需判定
      [ -e "$AU_ROOT/current" ] && cur_dir="$AU_ROOT/current"
      [ -n "$cur_dir" ] && cur_ta="$(ta_set "$cur_dir")"
      if [ -z "$cur_ta" ]; then
        warn "无基线 TA 可比对(flat/首装),跳过 TA 内容门 —— TA 版本化落地后强制(设计 §9)"
      elif [ "$bundle_ta" != "$cur_ta" ]; then
        die "bundle 实际 TA 集合与当前不一致(增/删/改)—— 拒绝(apply 不放行 TA,决策 D)。TA 更新走 OOB / 专门流程。"
      fi
    fi
  fi

  mkdir -p "$AU_ROOT/releases"
  apply_version "$ac_ver" "$bundle"
}

# 枚举一个目录树下所有 *.ta 的「文件名:sha256」有序集合(TA 由 UUID 文件名唯一标识)。
# 用于 TA 内容门:比较 bundle 与当前 current 的整套 TA(不只第一个)。
ta_set() { # <dir>
  local d="$1" p
  [ -e "$d" ] || return 0
  find -L "$d" -type f -name '*.ta' 2>/dev/null | sort | while IFS= read -r p; do
    printf '%s:%s\n' "$(basename "$p")" "$(sha256_of "$p")"
  done | sort
}

# ── 富通知 + 去重(设计:通知含版本/变动/安全级别/是否含 TA)──────────
# 去重 key = version+hash+severity+ta_changed+reqta+notes+metadata_version(任一变化重推,
# codex Medium-4:notes/reqta 变化=消息含义变化,也应重推)。
notify_update() { # <cur> <ver> <severity> <notes> <ta_changed> <reqta> <hash>
  local cur="$1" ver="$2" sev="$3" notes="$4" ta="$5" reqta="$6" hash="$7"
  local key="${ver}|${hash}|${sev}|${ta}|${reqta}|${notes}|${MVER}"
  if [ "$key" = "$(state_get notified_key)" ]; then
    log "该更新已通知过(去重 key 未变),跳过"; return 0
  fi
  local sevtag
  case "$sev" in
    critical) sevtag="🔴 CRITICAL 安全更新" ;;
    high)     sevtag="🔴 HIGH 安全补丁" ;;
    medium)   sevtag="🟠 MEDIUM" ;;
    low)      sevtag="🟡 LOW" ;;
    *)        sevtag="常规更新" ;;
  esac
  local tatag
  if [ "$ta" = "true" ]; then tatag="是 ⚠️ 含 TA 变更(走专门流程,不在线一键)"; else tatag="否(纯 CA)"; fi
  local msg="有新版本可用:$cur → $ver
安全级别: $sevtag
变动: ${notes:-（无说明）}
TA: $tatag"
  [ -n "$reqta" ] && msg="$msg
需要 TA ≥ $reqta"
  msg="$msg
应用: ssh 进板跑  aastar-node-updater apply $ver
(node=$AU_NODE_ID)"
  # 只在确认送达后才写去重 key(codex High-3):Telegram 一次瞬时故障不能让同一
  # 安全更新以后永远不再推。未送达 → 不写 key,下次 timer 重试。
  if notify_send info "$msg"; then
    state_set notified_key "$key"
  else
    warn "通知未送达 —— 暂不写去重 key,下次 check 会重推"
  fi
}

# ── check:主流程 ───────────────────────────────────────────────────
cmd_check() {
  # env 已由 main 最先 source(含 Telegram 凭据)。
  STRICT_FETCH=0                # check=定时器:网络失败静默重试(内部控制位)
  flush_pending_notify         # 先投递上一轮 recovery 等挂起的告警(网络此时已起,pr-daemon 三轮 #3)
  need jq; need curl
  acquire_lock
  state_init

  # TA 内容门授权位(codex High-2):check 只在 TA_AUTO_UPDATE=on 时才允许实际 TA 变更;
  # 否则 download_verify_apply 若发现 bundle 夹带变化的 TA(manifest 误标)会拒绝。
  TA_OK=$([ "${TA_AUTO_UPDATE:-off}" = on ] && echo 1 || echo 0)
  WORK="$(mktemp -d)"
  load_manifest        # 拉取 + 验签 + schema + 新鲜度 + 防回滚 → MANIFEST/FLOOR/CUR/MVER

  # 遍历所有候选(version>CUR, >=FLOOR, min_version 满足),逐个按策略判定:
  #   - apply 目标 = 能被 apply 的「最高」版本(不是版本最高的那个)—— 否则一个更高的
  #     非安全 minor 会挡住本该自动打的安全 patch,架空 security 策略(PR#191 High-1)。
  #   - notify 目标 = 所有候选里的最高版本,并带上它的 severity/notes/ta(富通知)。
  # 用 US(\037,非空白)作分隔符:tab 是 IFS 空白会折叠空字段,导致列错位。
  local US=$'\037'
  local ac_ver="" ac_tarball="" ac_sha=""      # apply 候选(能自动应用的最高版)
  local nc_ver="" nc_sev="none" nc_notes="" nc_ta="false" nc_hash="" nc_reqta=""  # notify 候选
  local r_ver r_sec r_auto r_ta r_min r_reqta r_tarball r_sha r_canary r_sev r_notes act
  while IFS="$US" read -r r_ver r_sec r_auto r_ta r_min r_reqta r_tarball r_sha r_canary r_sev r_notes; do
    [ -z "$r_ver" ] && continue
    ver_gt "$r_ver" "$CUR" || continue
    ver_ge "$r_ver" "$FLOOR" || continue
    ver_ge "$CUR" "$r_min" || continue           # 上升路径:当前须 >= 该版本要求的 min
    if [ -z "$nc_ver" ] || ver_gt "$r_ver" "$nc_ver"; then
      nc_ver="$r_ver"; nc_sev="$r_sev"; nc_notes="$r_notes"; nc_ta="$r_ta"; nc_hash="$r_sha"; nc_reqta="$r_reqta"
    fi
    act="$(decide_action "$CUR" "$r_ver" "$r_sec" "$r_auto" "$r_ta" "$r_canary" "$r_reqta")"
    if [ "$act" = "apply" ] && { [ -z "$ac_ver" ] || ver_gt "$r_ver" "$ac_ver"; }; then
      ac_ver="$r_ver"; ac_tarball="$r_tarball"; ac_sha="$r_sha"
    fi
  done <<EOF
$(jq -r '.releases[] | [(.version),(.security//false|tostring),(.auto_apply_allowed//false|tostring),(.ta_changed//false|tostring),(.min_version//"0.0.0"),(.requires_ta_version//""),(.tarball//""),(.sha256//""),((.canary_ring//[])|join(",")),(.severity//"none"),(.notes//"")] | join("")' "$MANIFEST")
EOF

  # rollback_floor 越界告警(当前版本已低于 floor)
  if ! ver_ge "$CUR" "$FLOOR"; then
    notify warn "当前版本 $CUR 低于 rollback_floor $FLOOR(存在已知漏洞)—— 应尽快升级(node=$AU_NODE_ID)"
  fi

  if [ -z "$nc_ver" ]; then log "无更高候选版本(current=$CUR)"; return 0; fi

  # 无可自动应用的候选 → 只发富通知(版本/变动/安全级别/是否含 TA + 去重)
  if [ -z "$ac_ver" ]; then
    notify_update "$CUR" "$nc_ver" "$nc_sev" "$nc_notes" "$nc_ta" "$nc_reqta" "$nc_hash"
    return 0
  fi
  log "apply 目标 $ac_ver(候选最高 $nc_ver,current=$CUR)"
  # 若存在比 apply 目标更高、但不自动应用的版本,也富通知一声(带 severity)
  if ver_gt "$nc_ver" "$ac_ver"; then
    notify_update "$CUR" "$nc_ver" "$nc_sev" "$nc_notes" "$nc_ta" "$nc_reqta" "$nc_hash"
  fi

  # 应用(下载 + sha256 + 可选验签 + tar 加固 + crash-safe + 健康门 + 回滚)
  download_verify_apply "$ac_ver" "$ac_tarball" "$ac_sha"
}

# ── apply <ver>:显式指定版本应用(运维已决定)──────────────────────
# 越过 policy 门(人已拍板),但绝不越过密码学 + 防回滚 + 兼容 + TA 门。
# version 只作「在已验签 manifest 里选哪条」的索引;hash/URL 全取自已验签数据。
cmd_apply() {
  local want="${1:-}"
  # env 已由 main 最先 source(含 Telegram 凭据),故这两个早期 die 的告警也能送达(pr-daemon 三轮 #2)。
  [ -n "$want" ] || die "用法: aastar-node-updater apply <version>"
  ver_valid "$want" || die "版本号非法(需 x.y.z): $want"
  # apply 的安全姿态:钉死在 main source 之后,updater.env 不得覆盖(pr-daemon #3):
  STRICT_FETCH=1                      # 下载失败必须硬失败(不静默误判成功)
  LOCK_FATAL=1                        # 锁竞争 → die,不静默 no-op(pr-daemon #2)
  TA_OK=0                             # apply 永不放行 TA 变更(决策 D,pr-daemon #3/#4)
  need jq; need curl
  acquire_lock
  state_init
  WORK="$(mktemp -d)"
  load_manifest

  # 在已验签 manifest 里精确取该版本(version 只作索引)
  local US=$'\037' line
  line="$(jq -r --arg v "${want#v}" '
    first(.releases[] | select((.version|ltrimstr("v"))==$v)
    | [(.version),(.security//false|tostring),(.auto_apply_allowed//false|tostring),(.ta_changed//false|tostring),(.min_version//"0.0.0"),(.requires_ta_version//""),(.tarball//""),(.sha256//""),(.severity//"none")]
    | join(""))' "$MANIFEST")"
  [ -n "$line" ] || die "manifest 里没有版本 $want(可用:$(jq -r '.releases[].version' "$MANIFEST" | tr '\n' ' '))"
  local r_ver r_sec r_auto r_ta r_min r_reqta r_tarball r_sha r_sev
  IFS="$US" read -r r_ver r_sec r_auto r_ta r_min r_reqta r_tarball r_sha r_sev <<EOF
$line
EOF

  # 安全门(显式 apply 越过 policy,但这些都不越过):
  ver_gt "$r_ver" "$CUR"   || die "$want 不高于当前 $CUR —— 拒绝(降级请走 OOB break-glass)"
  ver_ge "$r_ver" "$FLOOR" || die "$want 低于 rollback_floor $FLOOR(有已知漏洞)—— 拒绝"
  ver_ge "$CUR" "$r_min"   || die "当前 $CUR 不满足 $want 要求的 min_version $r_min —— 需先升级中间版本"
  if [ "$r_ta" = "true" ]; then
    die "$want 含 TA 变更(ta_changed=true)。TA 更新不走在线一键(RSA-4096 签名 + secure storage 迁移;apply_version 也不装 TA 到 OP-TEE 路径,会 CA/TA 不一致)——请走 OOB / 专门流程(决策 D)。"
  fi
  # 兼容门:要求的 TA 比「当前真实 TA 版本」新且本次不换 TA → 拒绝(pr-daemon #3:
  # 用真实 TA 版本,不是 CA 版本近似;TA 版本未知也 fail-closed 拒绝,不 fail-open)。
  if [ -n "$r_reqta" ] && [ "$r_ta" != "true" ]; then
    local cur_ta_ver; cur_ta_ver="$(ta_version)"
    if [ -z "$cur_ta_ver" ] || ! ver_valid "$cur_ta_ver"; then
      die "$want 要求 TA ≥ $r_reqta,但当前 TA 版本未知(设 AU_TA_VERSION 或写 \$AU_STATE_DIR/ta-version)—— fail-closed 拒绝"
    fi
    ver_ge "$cur_ta_ver" "$r_reqta" || die "$want 要求 TA ≥ $r_reqta,当前 TA $cur_ta_ver 不满足 —— 拒绝(需先升级 TA)"
  fi
  log "显式应用 $want(severity=$r_sev, ta_changed=$r_ta, current=$CUR)"
  notify info "开始应用 $CUR → $want(手动 apply,node=$AU_NODE_ID)"
  download_verify_apply "$r_ver" "$r_tarball" "$r_sha"
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
  # 兼容性门(PR#191 Medium + pr-daemon #3):CA 要求的 TA 比「当前真实 TA 版本」新,且本次
  # 不换 TA → 只通知,不自动升到与旧 TA 不兼容的 CA。用真实 TA 版本(非 CA 版本近似);
  # TA 版本未知也 fail-closed → notify(不 fail-open 自动应用)。
  if [ -n "$reqta" ] && [ "$ta" != "true" ]; then
    local tav; tav="$(ta_version)"
    if [ -z "$tav" ] || ! ver_valid "$tav" || ver_gt "$reqta" "$tav"; then echo notify; return; fi
  fi
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
  # 最先 source env(带 Telegram 凭据)—— 这样连 argv 校验/缺参这些最早的 die 告警也能送达
  # (pr-daemon 三轮 #2:之前 cmd_apply/main 的早期 die 在 source 之前,告警静默)。
  load_policy_env
  local cmd="${1:-check}"; shift 2>/dev/null || true
  # 位置参数收集到 POS[];无选项(--allow-ta 已移除,决策 D:apply 不放行 TA)。
  local POS=()
  local a
  for a in "$@"; do
    case "$a" in
      --*) die "未知选项 $a" ;;
      *) POS+=("$a") ;;
    esac
  done
  local npos="${#POS[@]}"
  case "$cmd" in
    check|recovery|status)
      [ "$npos" -eq 0 ] || die "$cmd 不接受额外参数:${POS[*]}"
      "cmd_$cmd" ;;
    apply)
      [ "$npos" -eq 1 ] || die "用法: $0 apply <ver>(恰好一个版本参数)"
      cmd_apply "${POS[0]}" ;;
    *) die "用法: $0 {check | apply <ver> | recovery | status}" ;;
  esac
}
main "$@"
