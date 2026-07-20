#!/usr/bin/env bash
# dk2.sh — DK2(STM32MP157F-DK2 / node2 / dvt2)串口访问工具(macOS)。
#
# DK2 串口 = micro-USB 插 CN11(板载 ST-LINK VCP)。⚠️必须【数据线】(充电线没 D+/D- 不出串口)。
# 板子通电 + ST-LINK 枚举 → macOS 出 /dev/cu.usbmodemXXXX。串口自动 root 登录(无密码)。
#
#   dk2.sh find                 找到并打印 DK2 的串口设备路径
#   dk2.sh console              screen 交互连接(退出: Ctrl-a 再 k,y)
#   dk2.sh run "<命令>" [秒]     在 DK2 上跑命令、打印回显(串口,非交互)
#   dk2.sh put <本地> <远程>     串口传小文件(config 级;大文件走网络)
#
# 覆盖: DK2_SERIAL=/dev/cu.usbmodemXXX dk2.sh ...   跳过自动查找。
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PY="$HERE/dk2-serial.py"
BOARDB_HINT="${BOARDB_SERIAL_HINT:-5B6D}"   # 板B 的 MCU-Link 串口前缀,查找时排除

die() { echo "dk2: $*" >&2; exit 1; }

find_dev() {
  [ -n "${DK2_SERIAL:-}" ] && { echo "$DK2_SERIAL"; return; }
  # 确认 DK2 的 ST-LINK 已枚举(数据线 + 通电)
  ioreg -p IOUSB -l -w0 2>/dev/null | grep -qi "STM32 STLink\|ST-LINK" \
    || die "没枚举到 DK2 的 ST-LINK —— 检查: micro-USB 是【数据线】、插 CN11、板子通电"
  # 候选 = 所有 cu.usbmodem,排除板B的 MCU-Link(5B6D)
  local d cand=""
  for d in /dev/cu.usbmodem*; do
    [ -e "$d" ] || continue
    case "$d" in *"$BOARDB_HINT"*) continue ;; esac
    cand="$d"; break
  done
  [ -n "$cand" ] || die "找到 ST-LINK 但没对应 /dev/cu.usbmodem(排除板B $BOARDB_HINT 后为空;用 DK2_SERIAL= 指定)"
  echo "$cand"
}

cmd="${1:-find}"
case "$cmd" in
  find)
    DEV="$(find_dev)"; echo "$DEV" ;;
  console)
    DEV="$(find_dev)"; echo "连接 $DEV @115200(退出: Ctrl-a k y)"; exec screen "$DEV" 115200 ;;
  run)
    [ $# -ge 2 ] || die "用法: dk2.sh run \"<命令>\" [超时秒]"
    DEV="$(find_dev)"; python3 "$PY" "$DEV" run "$2" "${3:-8}" ;;
  put)
    [ $# -ge 3 ] || die "用法: dk2.sh put <本地> <远程>"
    DEV="$(find_dev)"; python3 "$PY" "$DEV" put "$2" "$3" ;;
  *)
    die "未知子命令: $cmd (find|console|run|put)" ;;
esac
