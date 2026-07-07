#!/usr/bin/env bash
# dvt-unlock.sh — 在板上运行。断电/重启后 BLS keystore 密码从 tmpfs 丢失,
# 运维用本脚本重新输入密码 → 写入 tmpfs(/run,内存)→ 启动 DVT。
#
# 用法(在板上): bash /opt/dvt-build/dvt-unlock.sh
#   或远程一次性: ssh root@<board> 'bash /opt/dvt-build/dvt-unlock.sh'  (会交互提示输密码)
#
# 崩溃(非断电)时无需本脚本:systemd Restart=on-failure 会从仍在的 /run/dvt/pass 自动重启。
set -eu

mkdir -p /run/dvt && chmod 700 /run/dvt
printf "NODE_KEY_PASSPHRASE: " >&2
read -rs PASS; echo >&2
[ -n "$PASS" ] || { echo "空密码,取消"; exit 2; }
printf 'NODE_KEY_PASSPHRASE=%s\n' "$PASS" > /run/dvt/pass
chmod 600 /run/dvt/pass
unset PASS

systemctl reset-failed dvt.service 2>/dev/null || true
systemctl start dvt.service
# 端口从 dvt.env 读(与 community.toml 配置一致),不硬编码 8080
PORT="$(grep -E '^PORT=' /opt/dvt-build/dvt.env 2>/dev/null | cut -d= -f2 | tr -d ' ')"
PORT="${PORT:-8080}"
echo "▶ 已启动,等 25s 验证(:$PORT)..."
sleep 25
if curl -sf -m5 "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then
  echo "✅ DVT 已解锁并运行(keystore 解密成功)"
  grep -aiE "encrypted keystore" /opt/dvt-build/dvt.log 2>/dev/null | tail -1 | sed 's/\x1b\[[0-9;]*m//g'
else
  echo "❌ 未起来 —— 密码可能错。查:journalctl -u dvt.service -n 20 / tail /opt/dvt-build/dvt.log"
  exit 1
fi
