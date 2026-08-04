#!/usr/bin/env bash
# Power off imx93 board A (kms.aastar.io 生产 KMS + dvt1) — clean shutdown over Tailscale SSH.
#
# A 板已送机房,走 SSH(Tailscale host `mx93` = 100.121.187.3);本地串口够不到,不再用串口脚本。
#
# On next power-on (与 B 板不同):
#   • kms1/kms-api 自启  → kms.aastar.io 回来
#   • dvt1 是 KMS-TEE 托管节点(BLS 私钥 + operator 都在 TEE),随 kms 自启,**无需手动解锁**
#     —— 这点和 B 板的独立 dvt3(本地加密 keystore,断电要 dvt-unlock 重输密码)不一样。
#
# Usage: bash mx93a-poweroff.sh [ssh-host]   (默认 host: mx93)
set -uo pipefail
HOST="${1:-mx93}"

# 关机核心借鉴已验证生效的 poweroff-imx93.sh：SSH 里**同步**跑 `systemctl stop ...; sync; poweroff`。
# ⚠️ 不后台化——SSH 断开时后台子进程会被 SIGHUP 杀掉,poweroff 反而没执行(旧版 bug)。
echo "== board A ($HOST) — 关机前状态 =="
if ! ssh -o ConnectTimeout=10 "$HOST" \
     'echo "  kms-api=$(systemctl is-active kms-api) dvt=$(systemctl is-active dvt) cloudflared=$(systemctl is-active cloudflared)"; echo "  $(uptime)"' 2>/dev/null; then
  echo "SSH 不通 —— 板子可能已关机/未上电（A 板在机房）。"; exit 1
fi

echo "== 同步关机（stop 服务 → sync → poweroff；ssh 断开=关机已触发，属正常）=="
ssh -o ConnectTimeout=8 "$HOST" 'systemctl stop kms-api dvt cloudflared 2>/dev/null; sync; poweroff' 2>/dev/null
echo "   poweroff 已下达（ssh rc=$? —— 非 0 是机器 halt 导致连接断开，正常）"

echo "== 等 20s 后探测确认 =="
sleep 20
if ssh -o ConnectTimeout=6 -o BatchMode=yes "$HOST" 'echo up' 2>/dev/null | grep -q up; then
  echo "⚠️  板子还在线 —— 关机可能被某服务卡住。手动: ssh $HOST 'poweroff -f'，或看 LED。"
  exit 2
else
  echo "✅ board A SSH 已不通 = 已关机。确认 LED 灭即可拔电。"
fi
