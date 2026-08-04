#!/usr/bin/env bash
# 串口带外「自拉 release 升级」一键工具 —— 板子上电但 SSH/tailscale 不通时,
# 运维在 Mac 端驱动板子从 GitHub Release 自拉指定版本、验签校验、原子替换 CA、
# 烟测,失败自动回滚。是 updater(kms/deploy/updater/,自主/manifest 路径)的
# **手动带外对应物**:updater 管日常自动升级,本脚本管「够不到、要手动救+升」。
#
# 信任模型(两段验证,和手动流程一致):
#   Mac 端  : gh 下载 release → minisign 验签(pin 死可信公钥,authenticity)
#              + sha256 声明==实际(integrity)。不信 release 自带的 updater.pub(循环信任)。
#   板子端  : curl 同一 tarball → sha256 == Mac 已验证的哈希(在途完整性)
#              + tar 加固(拒绝绝对路径/../symlink/hardlink/设备节点/多顶层)。
#   板上无 minisign,故 authenticity 必须在 Mac 端完成 —— 本脚本正是 Mac 端驱动。
#
# 用法:
#   ./serial-selfupdate.sh <serial-device> <release-tag>
#   ./serial-selfupdate.sh /dev/cu.usbmodem…831 airaccount-node-v0.29.1
#   MX93_SERIAL=/dev/cu.usbmodem…831 ./serial-selfupdate.sh '' airaccount-node-v0.29.1
#
# 选项(环境变量):
#   ENSURE_NET=1        升级前先确保板子有外网;没有则做 WiFi 救网(见下)
#   WIFI_IFACE=mlan0    WiFi 接口(NXP 板是 mlan0 不是 wlan0)
#   WIFI_SELECT_ID=0    救网时 wpa_cli select_network 的 id(0=@JumboPlusIoT5GHz,板 A/B 已验证)
#   PORTAL_MARKER=...   额外烟测:断言 /portal 命中该 grep 模式(默认 'data-lang=.th.' 三语门;设空跳过)
#   EXPECT_VERSION=...  额外烟测:断言 /version 的 version/build == 此值(默认不校验,因 portal-only 版号不变)
#   REPO=AAStarCommunity/AirAccount
#   MX93_HOSTNAME=imx93-11x11-lpddr4x-frdm   期望主机名(防驱动错板)
#   REMOTE_BIN=/opt/airaccount/kms-api-server   板上目标 CA 路径
#   KMS_SERVICE=kms-api                          板上服务名
#
# 依赖:python3 + pyserial + jq + gh + minisign(均在 Mac 端)。
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SR="$HERE/serial-run.py"

DEV="${1:-${MX93_SERIAL:-}}"
TAG="${2:-${RELEASE_TAG:-}}"
REPO="${REPO:-AAStarCommunity/AirAccount}"
HOST="${MX93_HOSTNAME:-imx93-11x11-lpddr4x-frdm}"
REMOTE_BIN="${REMOTE_BIN:-/opt/airaccount/kms-api-server}"
KMS_SERVICE="${KMS_SERVICE:-kms-api}"
PORTAL_MARKER="${PORTAL_MARKER-data-lang=.th.}"   # 用 - 不用 :- ,允许显式设空跳过
EXPECT_VERSION="${EXPECT_VERSION:-}"
ENSURE_NET="${ENSURE_NET:-0}"
WIFI_IFACE="${WIFI_IFACE:-mlan0}"
WIFI_SELECT_ID="${WIFI_SELECT_ID:-0}"

# ── pin 死可信 minisign 公钥(信任锚;绝不用 release 自带的 updater.pub)──────
# key ID B4D4EC2546A19EB2(= kms/deploy/updater/updater-pubkey.pub 生产发布链公钥;
# 换轮公钥时这里与 updater-pubkey.pub 同步改)。
TRUSTED_PUBKEY_LINE='RWSynqFGJezUtHE8RGgt/hza4GLIjbaivYjnBKwuQ+liM52mXNbgVzAo'

# ── 前置检查 ─────────────────────────────────────────────────────────
fail() { echo "✗ $*" >&2; exit 1; }
[ -n "$DEV" ]  && [ "$DEV" != auto ] || { echo "✗ 必须显式指定串口设备(=物理板身份,防升错板)" >&2;
  echo "  候选:" >&2; python3 "$SR" --list >&2; exit 2; }
[ -e "$DEV" ] || fail "串口设备不存在: $DEV"
[ -n "$TAG" ] || fail "必须给 release tag(如 airaccount-node-v0.29.1)"
for t in python3 jq gh minisign; do command -v "$t" >/dev/null || fail "Mac 端缺依赖 $t"; done

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
STAMP="$(date +%Y%m%d-%H%M%S)"

echo "▶ 串口自拉升级  dev=$DEV  tag=$TAG  repo=$REPO"

# ── 1) Mac 端:下载 release + 验签(authenticity)+ sha256(integrity)──────
echo "── [Mac] 下载 + 验签 ──"
gh release download "$TAG" -R "$REPO" -D "$WORK/rel" \
  -p '*.tar.gz' -p '*.tar.gz.sha256' -p '*.tar.gz.minisig' 2>/dev/null \
  || fail "gh release download 失败(tag 不存在 / 无网 / 无权限)"
TARBALL="$(find "$WORK/rel" -name '*.tar.gz' | head -1)"
[ -n "$TARBALL" ] || fail "release 里没有 tar.gz 资产"
BASENAME="$(basename "$TARBALL")"
SHAFILE="$TARBALL.sha256"; SIGFILE="$TARBALL.minisig"
[ -f "$SHAFILE" ] || fail "release 缺 $BASENAME.sha256"
[ -f "$SIGFILE" ] || fail "release 缺 $BASENAME.minisig(无签名一律拒绝)"

# 写 pin 公钥到临时文件,验签
echo "untrusted comment: pinned aastar updater pubkey" > "$WORK/trusted.pub"
echo "$TRUSTED_PUBKEY_LINE" >> "$WORK/trusted.pub"
minisign -Vm "$TARBALL" -p "$WORK/trusted.pub" -x "$SIGFILE" >/dev/null 2>&1 \
  || fail "minisign 验签失败 —— 签名不是 pin 的可信公钥所签,拒绝(疑似投毒/换源)"
echo "  ✓ minisign 验签通过(pin 公钥 ${TRUSTED_PUBKEY_LINE:0:16}…)"

DECLARED_SHA="$(awk '{print $1}' "$SHAFILE")"
ACTUAL_SHA="$(shasum -a 256 "$TARBALL" | awk '{print $1}')"
[ "$DECLARED_SHA" = "$ACTUAL_SHA" ] || fail "sha256 声明($DECLARED_SHA)!= 实际($ACTUAL_SHA)"
echo "  ✓ sha256 一致: $ACTUAL_SHA"

# release 资产下载 URL(板子端用)
TARBALL_URL="$(gh release view "$TAG" -R "$REPO" --json assets \
  -q ".assets[] | select(.name==\"$BASENAME\") | .url" 2>/dev/null | grep -E '^https' | head -1)"
[ -n "$TARBALL_URL" ] || fail "拿不到 tarball 下载 URL"

# ── 2) 板子端:guard(root shell + hostname,防升错板)───────────────────
echo "── [板] 身份守卫 ──"
guard="$(python3 "$SR" --dev "$DEV" --json 'whoami' 'hostname')" \
  || fail "串口执行失败(登录不上/设备占用)"
who=$(printf '%s' "$guard"  | jq -r '.[0].out'); who_rc=$(printf '%s' "$guard"  | jq -r '.[0].rc')
host=$(printf '%s' "$guard" | jq -r '.[1].out'); host_rc=$(printf '%s' "$guard" | jq -r '.[1].rc')
[ "$who_rc" = 0 ] && [ "$who" = root ] || fail "没拿到 root shell(whoami='$who' rc=$who_rc)—— 中止"
[ "$host_rc" = 0 ] && [ "$host" = "$HOST" ] || fail "hostname '$host' != 期望 '$HOST'(可能选错板;MX93_HOSTNAME 可覆盖)"
echo "  ✓ root@$host"

# ── 3) 可选:确保外网,不通则 WiFi 救网 ────────────────────────────────
if [ "$ENSURE_NET" = 1 ]; then
  echo "── [板] 联网自检 ──"
  net="$(python3 "$SR" --dev "$DEV" --timeout 15 --json \
    'curl -sfI -m 8 https://github.com >/dev/null && echo UP || echo DOWN' || true)"
  st=$(printf '%s' "$net" | jq -r '.[0].out' | tr -d '[:space:]')
  if [ "$st" = UP ]; then
    echo "  ✓ 外网可达"
  else
    echo "  ! 外网不可达 → WiFi 救网(iface=$WIFI_IFACE select_network $WIFI_SELECT_ID)"
    python3 "$SR" --dev "$DEV" --timeout 40 \
      "wpa_cli -i $WIFI_IFACE disable_network all; wpa_cli -i $WIFI_IFACE enable_network $WIFI_SELECT_ID; wpa_cli -i $WIFI_IFACE select_network $WIFI_SELECT_ID" \
      "sleep 12; wpa_cli -i $WIFI_IFACE status | grep -E 'wpa_state|ssid='" \
      "udhcpc -i $WIFI_IFACE -n -q 2>&1 | tail -3" \
      "wpa_cli -i $WIFI_IFACE save_config" >&2 || true
    net2="$(python3 "$SR" --dev "$DEV" --timeout 20 --json \
      'curl -sfI -m 10 https://github.com >/dev/null && echo UP || echo DOWN')"
    st2=$(printf '%s' "$net2" | jq -r '.[0].out' | tr -d '[:space:]')
    [ "$st2" = UP ] || fail "救网后仍无外网 —— 人工排查(scan_results / 信号 / 门户)"
    echo "  ✓ 救网成功,外网已通"
    python3 "$SR" --dev "$DEV" --timeout 15 'systemctl restart tailscaled 2>/dev/null; sleep 4; tailscale ip -4 2>/dev/null || true' >&2 || true
  fi
fi

# ── 4) 板子端:下载 + sha256 + tar 加固 + 解压 ─────────────────────────
# 说明:tarball 的 authenticity+integrity 已在 Mac 端用 pin 公钥 minisign 验过,
# 且下面板上 sha256 == 那份已验证哈希 —— 信任已建立。板上 tar 检查=纵深防御。
# 捕获一律 `|| true`:serial-run.py 若非零退出,不能让 set -e 静默杀掉本脚本
# 的诊断分支(否则看不到到底哪步坏)。
echo "── [板] 下载 + 校验 + 解压 ──"
dl="$(python3 "$SR" --dev "$DEV" --timeout 90 --json \
  "rm -rf /tmp/su && mkdir -p /tmp/su && cd /tmp/su && curl -fL -m 75 -o node.tgz '$TARBALL_URL' -w 'http=%{http_code}\n' 2>/dev/null" \
  "cd /tmp/su && echo '$DECLARED_SHA  node.tgz' | sha256sum -c - && echo SHA_OK" \
  "cd /tmp/su && tar -tzf node.tgz | grep -Eq '^/|(^|/)\\.\\./' && echo BADPATH || echo PATH_OK" \
  "cd /tmp/su && tar xzf node.tgz 2>/dev/null && test -f airaccount-node-*/kms/kms-api-server && echo EXTRACT_OK" \
  || true)"
[ -n "$dl" ] || fail "串口下载阶段无返回(超时/串口占用?)"
dl_http=$(printf '%s' "$dl" | jq -r '.[0].out' | grep -oE 'http=[0-9]+' | head -1 | cut -d= -f2)
sha_out=$(printf '%s' "$dl" | jq -r '.[1].out'); path_out=$(printf '%s' "$dl" | jq -r '.[2].out')
ext_out=$(printf '%s' "$dl" | jq -r '.[3].out')
[ "${dl_http:-0}" = 200 ] || fail "板子下载失败(http=${dl_http:-?})"
printf '%s' "$sha_out"  | grep -q SHA_OK     || fail "板子端 sha256 校验失败 —— 在途损坏/被篡改,拒绝"
printf '%s' "$path_out" | grep -q PATH_OK    || fail "tar 含绝对路径或 .. —— 拒绝"
printf '%s' "$ext_out"  | grep -q EXTRACT_OK || fail "解压失败/缺 kms/kms-api-server —— 拒绝"
echo "  ✓ 下载 http200 · sha256 OK · 路径安全 · 解压 OK"

# TA 警示:本手动工具只换 CA;bundle 若含 TA(*.ta)不静默处理(TA 牵动 secure storage)
ta="$(python3 "$SR" --dev "$DEV" --json \
  "find /tmp/su -name '*.ta' 2>/dev/null | head -1 || true" || true)"
ta_path=$(printf '%s' "$ta" | jq -r '.[0].out' | tr -d '[:space:]')
if [ -n "$ta_path" ] && [ "$ta_path" != "null" ]; then
  echo "  ! 注意:bundle 含 TA($ta_path)。本 OOB 工具只替换 CA,不动 TA。"
  echo "    TA 更新请走专门流程(RSA-4096 签名 + secure storage 迁移),勿用本脚本。"
fi

# 从捕获的 out 里抽「TAG<value>」—— 板子会往串口异步打 TA 生命周期噪声
# ([+] TA close/create/open session),会混进命令 out。故所有关键值都用唯一
# 标记包裹再取,正则容忍周围噪声(否则 systemctl is-active 会被打成
# '[+]TAcreate…active' != 'active' 触发假回滚)。
tagval() { printf '%s' "$1" | grep -oE "$2<[^>]*>" | head -1 | sed -E "s/.*<([^>]*)>.*/\1/"; }

# ── 5) 备份 + 停 + 换 + 启 ────────────────────────────────────────────
echo "── [板] 备份 → 替换 → 重启 ──"
BAK="$REMOTE_BIN.bak-$STAMP"
swap="$(python3 "$SR" --dev "$DEV" --timeout 40 --json \
  "cp -a '$REMOTE_BIN' '$BAK' && printf 'BAK<%s>\\n' \"\$(stat -c%s '$BAK')\"" \
  "systemctl stop $KMS_SERVICE && install -m755 -o root -g root /tmp/su/airaccount-node-*/kms/kms-api-server '$REMOTE_BIN' && printf 'INS<%s>\\n' \"\$(stat -c%s '$REMOTE_BIN')\"" \
  "systemctl start $KMS_SERVICE; sleep 5; printf 'ACT<%s>\\n' \"\$(systemctl is-active $KMS_SERVICE)\"" || true)"
[ -n "$swap" ] || fail "串口替换阶段无返回(超时/串口占用?)—— 请检查板上 $KMS_SERVICE 与 $REMOTE_BIN"
bak_rc=$(printf '%s'  "$swap" | jq -r '.[0].rc'); bak_sz=$(tagval "$(printf '%s' "$swap" | jq -r '.[0].out')" BAK)
ins_rc=$(printf '%s'  "$swap" | jq -r '.[1].rc'); ins_sz=$(tagval "$(printf '%s' "$swap" | jq -r '.[1].out')" INS)
act=$(tagval "$(printf '%s' "$swap" | jq -r '.[2].out')" ACT)
[ "$bak_rc" = 0 ] && [ -n "$bak_sz" ] || fail "备份失败 —— 中止(未动原文件)"
[ "$ins_rc" = 0 ] && [ -n "$ins_sz" ] || fail "替换失败 —— 中止(备份在 $BAK,服务可能已停,请人工检查)"
echo "  ✓ 备份 $bak_sz → $BAK   新版 $ins_sz 就位   服务=$act"

# ── 6) 烟测(失败自动回滚)──────────────────────────────────────────
echo "── [板] 烟测 ──"
rollback() {
  echo "  ↩ 回滚到 $BAK …" >&2
  python3 "$SR" --dev "$DEV" --timeout 30 --json \
    "systemctl stop $KMS_SERVICE; cp -a '$BAK' '$REMOTE_BIN' && systemctl start $KMS_SERVICE; sleep 5; systemctl is-active $KMS_SERVICE" >/dev/null 2>&1 || true
  fail "烟测未过,已尝试回滚(备份 $BAK 保留)。$1"
}
[ "$act" = active ] || rollback "服务未 active(实测='$act')"

# health + version(值 TAG 包裹躲 TA 噪声;/health 轮询等就绪 —— restart 后
# systemd 虽 active,但 HTTP /health 要等 TA session 重建数秒才 ready,不能只探一次)。
# 注意:板上 userland 极小,**没有 jq** —— 一律用 grep 解析,不依赖板上 jq。
smk="$(python3 "$SR" --dev "$DEV" --timeout 45 --json \
  "s=''; for i in \$(seq 1 10); do curl -s -m 5 localhost:3000/health | grep -q '\"status\":\"healthy\"' && { s=healthy; break; }; sleep 3; done; printf 'HL<%s>\\n' \"\$s\"" \
  "printf 'VER<%s>\\n' \"\$(curl -s -m 6 localhost:3000/version)\"" || true)"
health=$(tagval "$(printf '%s' "$smk" | jq -r '.[0].out')" HL)
verjson=$(tagval "$(printf '%s' "$smk" | jq -r '.[1].out')" VER)
[ "$health" = healthy ] || rollback "/health status='$health'"
echo "  ✓ /health = healthy"
echo "  · /version = $verjson"

if [ -n "$EXPECT_VERSION" ]; then
  got="$(printf '%s' "$verjson" | jq -r '.version // .build // empty' 2>/dev/null)"
  [ "$got" = "$EXPECT_VERSION" ] || rollback "/version '$got' != 期望 '$EXPECT_VERSION'"
  echo "  ✓ /version == $EXPECT_VERSION"
fi

if [ -n "$PORTAL_MARKER" ]; then
  pm="$(python3 "$SR" --dev "$DEV" --timeout 15 --json \
    "printf 'PM<%s>\\n' \"\$(curl -s -m 8 localhost:3000/portal | grep -c '$PORTAL_MARKER')\"" || true)"
  cnt=$(tagval "$(printf '%s' "$pm" | jq -r '.[0].out')" PM)
  [ "${cnt:-0}" -ge 1 ] 2>/dev/null || rollback "/portal 未命中 '$PORTAL_MARKER'(count=$cnt)"
  echo "  ✓ /portal 命中 '$PORTAL_MARKER'(count=$cnt)"
fi

# ── 7) 清理下载残留(保留 .bak 回滚点)────────────────────────────────
python3 "$SR" --dev "$DEV" 'rm -rf /tmp/su' >/dev/null 2>&1 || true

echo ""
echo "✔ 升级完成:$TAG 已装到 $REMOTE_BIN,烟测通过。"
echo "  回滚点:$BAK(确认稳定后可手动删)"
