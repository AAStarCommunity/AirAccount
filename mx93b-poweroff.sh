#!/usr/bin/env bash
# Power off imx93 board B (kms1.aastar.io + dvt3) — clean shutdown over Tailscale SSH.
#
# 关机核心借鉴已验证生效的 poweroff-imx93.sh：SSH 里**同步**跑 `systemctl stop ...; sync; poweroff`
# —— `poweroff` 直接 halt 机器,SSH 随之断开,ssh 报错(rc!=0)是正常的、正说明关成了。
# ⚠️ 不要后台化(`( ... ) &`)：SSH 断开时后台子进程被 SIGHUP 杀掉,poweroff 反而没执行(旧版 bug)。
#
# On next power-on:
#   • kms1 (kms-api) 自启 → kms1.aastar.io 回来
#   • dvt3 不自启：BLS keystore 密码在 tmpfs,断电即清 → ssh mx93b 后跑 `dvt-unlock` 重输密码
#
# Usage: bash mx93b-poweroff.sh [ssh-host]   (默认 host: mx93b)
set -uo pipefail
HOST="${1:-mx93b}"

echo "== board B ($HOST) — 关机前状态 =="
if ! ssh -o ConnectTimeout=10 "$HOST" \
     'echo "  kms-api=$(systemctl is-active kms-api) dvt=$(systemctl is-active dvt) cloudflared=$(systemctl is-active cloudflared)"; echo "  $(uptime)"' 2>/dev/null; then
  echo "SSH 不通 —— 板子可能已关机/未上电。"; exit 1
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
  echo "✅ board B SSH 已不通 = 已关机。确认 LED 灭即可拔电。"
fi
