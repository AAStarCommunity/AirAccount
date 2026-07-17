#!/usr/bin/env bash
#
# mac-mini-console-board-a.sh
# 在【与板A同处一地的 mac-mini】上运行。
# 把板A(MX93 / FRDM-IMX93)的 USB-C 调试串口挂成常驻、可远程 attach 的控制台。
# 板子断网(WiFi/Tailscale 挂了)时,仍能从 mac-mini(它在 Tailscale 上)screen 进串口救板。
#
# 用法:
#   ./mac-mini-console-board-a.sh up       # 启动/确保常驻控制台在跑(幂等)
#   ./mac-mini-console-board-a.sh attach   # 本机附着(退出用 Ctrl-b 再 d,不杀会话)
#   ./mac-mini-console-board-a.sh down     # 停掉常驻会话
#   ./mac-mini-console-board-a.sh list     # 列出当前 usbmodem 串口,用来核对 DEV_GLOB
#
# 远程(从我的 Mac 经 Tailscale):
#   ssh mac-mini -t '/opt/homebrew/bin/tmux attach -t board-a'   # Apple Silicon
#   ssh mac-mini -t '/usr/local/bin/tmux  attach -t board-a'     # Intel mac-mini
#
set -euo pipefail

# ===================== CONFIG(板A) =====================
BOARD="board-a"                        # tmux 会话名 + 日志文件名
DEV_GLOB="/dev/cu.usbmodem5B6D*"       # 板A MCU-Link 调试串口(USB-C)。先 list 核对!
BAUD=115200
LOG_DIR="${HOME}/airaccount-console-logs"
# =======================================================

TMUX_BIN="$(command -v tmux || true)"
cmd="${1:-up}"

list_devices() {
  echo "当前 /dev/cu.usbmodem* 串口设备:"
  /bin/ls /dev/cu.usbmodem* 2>/dev/null || echo "  (无 — 板子没插/没通电/线是充电线)"
  echo "本脚本 DEV_GLOB = $DEV_GLOB"
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
# ⚠️ 串口自动 root 登录,pipe-pane 把串口全部输出明文落盘。收权限 + 轮转:
#  - 日志目录 700(仅本人),日志文件 600。
#  - up 时若当前日志 >5MB 则轮转(留 .1/.2),避免无限增长。
#  - 别在被记录的串口里敲密钥(会明文进日志)——见 README。
LOG="$LOG_DIR/$BOARD.log"
mkdir -p "$LOG_DIR"; chmod 700 "$LOG_DIR"
if [ -f "$LOG" ] && [ "$(wc -c < "$LOG" 2>/dev/null || echo 0)" -gt 5242880 ]; then
  [ -f "$LOG.1" ] && mv -f "$LOG.1" "$LOG.2"
  mv -f "$LOG" "$LOG.1"
fi
touch "$LOG"; chmod 600 "$LOG" "$LOG.1" "$LOG.2" 2>/dev/null || true

if "$TMUX_BIN" has-session -t "$BOARD" 2>/dev/null; then
  echo "✅ $BOARD 控制台已在运行。"
  echo "   远程 attach: ssh mac-mini -t '$TMUX_BIN attach -t $BOARD'"
  exit 0
fi

# 监督循环:串口没出现就等它,出现就 screen 连;断开(板重启/拔插)后自动重连。
# 注意:内层 $DEV_GLOB/$BAUD/$LOG_DIR/$BOARD 在生成时展开;运行时变量用 \$ 转义。
read -r -d '' LOOP <<EOF || true
while true; do
  DEV=""
  for d in $DEV_GLOB; do [ -e "\$d" ] && { DEV="\$d"; break; }; done
  if [ -z "\$DEV" ]; then
    echo "[\$(date '+%F %T')] 等待板A串口出现 ($DEV_GLOB) ..."
    sleep 3; continue
  fi
  echo "[\$(date '+%F %T')] 连接 \$DEV @ $BAUD (退出附着: Ctrl-b d;不会断板)"
  screen "\$DEV" $BAUD || true
  echo "[\$(date '+%F %T')] 串口断开,3秒后重连…"
  sleep 3
done
EOF

"$TMUX_BIN" new-session -d -s "$BOARD" "$LOOP"
# 把控制台输出同时落盘(600),便于事后翻(哪怕没人 attach)
"$TMUX_BIN" pipe-pane -t "$BOARD" -o "cat >> \"$LOG\"" 2>/dev/null || true

echo "✅ 已启动 $BOARD 常驻控制台 (tmux 会话: $BOARD)"
echo "   本机 attach:  $TMUX_BIN attach -t $BOARD"
echo "   远程 attach:  ssh mac-mini -t '$TMUX_BIN attach -t $BOARD'"
echo "   日志:         $LOG_DIR/$BOARD.log"
echo "   退出附着(不杀会话): 先按 Ctrl-b 再按 d"
echo "   screen 内如需退屏: Ctrl-a 再按 k (会触发自动重连)"
