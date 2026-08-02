#!/usr/bin/env bash
# 串口优雅关闭板 B(mx93b)—— off-network(SSH/tailscale 不通)时用调试串口关机。
# 走 systemctl poweroff(停服 + 同步 + 根分区 remount RO),不是拔电。
#
# 用法:
#   ./mx93b-serial-poweroff.sh [serial-device]
#   设备优先级:参数 > $MX93B_SERIAL > auto(仅一个 usbmodem 时自动选)
#
# 板 B 调试口在 macOS 上通常是 /dev/cu.usbmodem<probe-serial>1(console;还有个 …3 是第二接口)。
# 依赖:python3 + pyserial(pip3 install pyserial)。
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
DEV="${1:-${MX93B_SERIAL:-auto}}"
RUN=(python3 "$HERE/serial-run.py" --dev "$DEV" --login root)

echo "▶ 板 B 串口关机 (dev=$DEV)"

# 1) 安全确认:必须先拿到 root shell,且确认是 imx93 板(避免对错设备乱发)
who="$("${RUN[@]}" 'whoami' 'hostname' 2>/dev/null || true)"
echo "$who"
echo "$who" | grep -q root || { echo "✗ 没拿到 root shell —— 中止(检查线/设备/是否已在别处占用串口)"; exit 1; }
echo "$who" | grep -qi imx93 || { echo "✗ hostname 不像 imx93 板 —— 中止(设备选错?)"; exit 1; }

# 2) 发关机并原始流读 25s 关机日志
"${RUN[@]}" --read-secs 25 'systemctl poweroff'

echo ""
echo "✔ 若上方出现 'reboot: Power down'(且之前有 'Unmounting file systems'/'re-mounted ... ro')即已干净关机。"
echo "  下次上电即恢复;串口 console 口通常仍枚举(调试芯片独立供电),但无输出。"
