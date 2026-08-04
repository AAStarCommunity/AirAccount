#!/usr/bin/env bash
#
# mac-mini-console-dk2.sh
# 在【与 DK2 同处一地(机房)的 mac-mini】上运行。
# 把 DK2(STM32MP157F-DK2 / dvt2)的 micro-USB CN11(ST-LINK VCP)串口挂成常驻、可远程 attach 的控制台。
# DK2 断网(WiFi/Tailscale 挂了)时,仍能从 mac-mini(它在 Tailscale 上)screen 进串口救板。
#
# ⚠️ DK2 串口 = micro-USB 插 CN11,必须【数据线】。板A/B 的串口是 USB-C(见 board-a/b 脚本)。
#    本脚本自动排除板A/B 的 5B6D MCU-Link,取 STM32 ST-LINK(DK2)。
#
# 用法:  ./mac-mini-console-dk2.sh [up|attach|down|list]
# 远程:  ssh mac-mini -t '/opt/homebrew/bin/tmux attach -t dk2'   (Intel 用 /usr/local/bin/tmux)
#
set -euo pipefail

# ===================== CONFIG(DK2) =====================
BOARD="dk2"                            # tmux 会话名 + 日志文件名
EXCLUDE="5B6D"                         # 排除板A/B 的 MCU-Link(它俩也在这台 mac-mini 上)
BAUD=115200
LOG_DIR="${HOME}/airaccount-console-logs"
# 覆盖: DK2_SERIAL=/dev/cu.usbmodemXXX ./mac-mini-console-dk2.sh up
# =======================================================

TMUX_BIN="$(command -v tmux || true)"
cmd="${1:-up}"

list_devices() {
  echo "当前 /dev/cu.usbmodem* 串口设备:"
  /bin/ls /dev/cu.usbmodem* 2>/dev/null || echo "  (无 — DK2 没插/没通电/线是充电线)"
  echo "DK2 = 排除 $EXCLUDE(板A/B MCU-Link)后的那个(STM32 ST-LINK)。ioreg 里应见 'STM32 STLink'。"
}

# 选 DK2 串口:优先 DK2_SERIAL;否则取第一个非 EXCLUDE 的 usbmodem。
pick_dev() {
  [ -n "${DK2_SERIAL:-}" ] && { echo "$DK2_SERIAL"; return; }
  local d
  for d in /dev/cu.usbmodem*; do
    [ -e "$d" ] || continue
    case "$d" in *"$EXCLUDE"*) continue ;; esac
    echo "$d"; return
  done
}

case "$cmd" in
  list)  list_devices; exit 0 ;;
  down)
    [ -n "$TMUX_BIN" ] || { echo "无 tmux"; exit 1; }
    "$TMUX_BIN" kill-session -t "$BOARD" 2>/dev/null && echo "已停止 $BOARD" || echo "$BOARD 没在跑"
    exit 0 ;;
  attach)
    [ -n "$TMUX_BIN" ] || { echo "无 tmux"; exit 1; }
    exec "$TMUX_BIN" attach -t "$BOARD" ;;
  up|"") : ;;
  *) echo "用法: $0 [up|attach|down|list]"; exit 2 ;;
esac

# ---------------- up ----------------
[ -n "$TMUX_BIN" ] || { echo "缺 tmux,请先: brew install tmux"; exit 1; }
# 串口自动 root 登录,pipe-pane 明文落盘 → 目录 700 / 文件 600 / >5MB 轮转。别在串口敲密钥。
LOG="$LOG_DIR/$BOARD.log"
mkdir -p "$LOG_DIR"; chmod 700 "$LOG_DIR"

# 先查常驻会话:已在跑就直接退出,**不碰日志** —— 否则会把正在写的 pipe-pane
# 日志从 fd 底下 rename 走,之后输出全落进 .1、$LOG 永久空掉。
if "$TMUX_BIN" has-session -t "$BOARD" 2>/dev/null; then
  echo "✅ $BOARD 控制台已在运行。远程 attach: ssh mac-mini -t '$TMUX_BIN attach -t $BOARD'"
  exit 0
fi

# 仅在确认没有常驻会话(即将新建)时才轮转日志,不会影响正在写的 fd。
if [ -f "$LOG" ] && [ "$(wc -c < "$LOG" 2>/dev/null || echo 0)" -gt 5242880 ]; then
  [ -f "$LOG.1" ] && mv -f "$LOG.1" "$LOG.2"
  mv -f "$LOG" "$LOG.1"
fi
touch "$LOG"; chmod 600 "$LOG" "$LOG.1" "$LOG.2" 2>/dev/null || true

# 监督循环:串口没出现就等它,出现就 screen 连;断开(板重启/拔插)后自动重连。
# 内层用 pick_dev(排除 5B6D);EXCLUDE/BAUD 生成时展开,运行时变量 \$ 转义。
read -r -d '' LOOP <<EOF || true
while true; do
  DEV=""
  for d in /dev/cu.usbmodem*; do
    [ -e "\$d" ] || continue
    case "\$d" in *"$EXCLUDE"*) continue ;; esac
    DEV="\$d"; break
  done
  if [ -z "\$DEV" ]; then
    echo "[\$(date '+%F %T')] 等待 DK2 串口出现 (非 $EXCLUDE 的 usbmodem) ..."
    sleep 3; continue
  fi
  echo "[\$(date '+%F %T')] 连接 \$DEV @ $BAUD (退出附着: Ctrl-b d;不会断板)"
  screen "\$DEV" $BAUD || true
  echo "[\$(date '+%F %T')] 串口断开,3秒后重连…"
  sleep 3
done
EOF

"$TMUX_BIN" new-session -d -s "$BOARD" "$LOOP"
"$TMUX_BIN" pipe-pane -t "$BOARD" -o "cat >> \"$LOG\"" 2>/dev/null || true

echo "✅ 已启动 $BOARD 常驻控制台 (tmux 会话: $BOARD)"
echo "   远程 attach:  ssh mac-mini -t '$TMUX_BIN attach -t $BOARD'"
echo "   日志:         $LOG"
echo "   退出附着(不杀会话): 先按 Ctrl-b 再按 d"
