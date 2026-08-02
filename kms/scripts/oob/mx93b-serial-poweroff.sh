#!/usr/bin/env bash
# 串口优雅关闭板 B(mx93b)—— off-network(SSH/tailscale 不通)时用调试串口关机。
# systemctl poweroff(停服 + sync + 根分区 remount RO),不是拔电。
#
# 用法:
#   ./mx93b-serial-poweroff.sh <serial-device>      # 破坏性操作:必须显式设备,不允许 auto
#   MX93B_SERIAL=/dev/cu.usbmodem…831 ./mx93b-serial-poweroff.sh
# 覆盖期望主机名(默认真实值):MX93B_HOSTNAME=... （板 A/B 同族主机名,靠“显式设备=物理板”区分)
#
# 依赖:python3 + pyserial + jq。
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SR="$HERE/serial-run.py"
DEV="${1:-${MX93B_SERIAL:-}}"
HOST="${MX93B_HOSTNAME:-imx93-11x11-lpddr4x-frdm}"

# Blocking 4:破坏性操作必须显式设备(设备=物理板身份)。禁 auto/空,避免关错板。
if [ -z "$DEV" ] || [ "$DEV" = auto ]; then
  echo "✗ 必须显式指定串口设备(破坏性操作,不允许 auto —— 防关错板)。" >&2
  echo "  用法: $0 <serial-device>   或设 MX93B_SERIAL" >&2
  echo "  候选设备:" >&2; python3 "$SR" --list >&2
  exit 2
fi
[ -e "$DEV" ] || { echo "✗ 设备不存在: $DEV" >&2; exit 2; }
command -v jq >/dev/null || { echo "✗ 缺 jq" >&2; exit 2; }

echo "▶ 板 B 串口关机 (dev=$DEV, 期望 hostname=$HOST)"

# Blocking 3:守卫用带 rc 的机器可读结果精确判定,不 grep 自由文本(启动日志/提示符能骗过 grep)。
# 守卫与关机用同一个显式 $DEV(不重新 glob),消除 TOCTOU。
guard="$(python3 "$SR" --dev "$DEV" --json 'whoami' 'hostname')" || {
  echo "✗ 串口执行失败(登录不上/设备占用?见上)—— 中止" >&2; exit 1; }
who=$(printf '%s' "$guard"  | jq -r '.[0].out'); who_rc=$(printf '%s' "$guard"  | jq -r '.[0].rc')
host=$(printf '%s' "$guard" | jq -r '.[1].out'); host_rc=$(printf '%s' "$guard" | jq -r '.[1].rc')
echo "  whoami='$who' (rc=$who_rc)   hostname='$host' (rc=$host_rc)"

[ "$who_rc" = 0 ] && [ "$who" = root ] || {
  echo "✗ 没拿到 root shell(whoami!=root 或 rc!=0)—— 中止" >&2
  echo "  可能:板子只在刷启动日志/未登录/串口被占用。绝不盲发关机。" >&2; exit 1; }
[ "$host_rc" = 0 ] && [ "$host" = "$HOST" ] || {
  echo "✗ hostname '$host' != 期望 '$HOST' —— 中止(设备可能选错;确认无误可用 MX93B_HOSTNAME 覆盖)" >&2
  exit 1; }

echo "✓ 确认活的 root shell 且 hostname 匹配 → 关机"
python3 "$SR" --dev "$DEV" --read-secs 25 'systemctl poweroff'
echo ""
echo "✔ 若上方出现 'reboot: Power down'(且有 'Unmounting file systems' / re-mounted … ro)即干净关机。"
echo "  下次上电即恢复;调试串口口通常仍枚举(调试芯片独立供电),但无输出。"
