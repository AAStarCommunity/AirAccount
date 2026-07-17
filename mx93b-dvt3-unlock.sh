#!/bin/sh
# mx93b-dvt3-unlock — bring the INDEPENDENT dvt3 node (B 板) back up after a power cycle.
#
# dvt3's BLS keystore passphrase (= DVT3_SECRET) lives ONLY in tmpfs (/run/dvt/pass),
# wiped on power loss — by design, so the plaintext key never touches flash. So dvt.service
# does NOT auto-start; you re-enter the passphrase by hand each boot.
#
# 用法（在板上跑，SSH 进去后）:
#     ssh mx93b            # Tailscale
#     dvt-unlock           # 装在板上 /usr/local/bin/dvt-unlock；然后输入密码（隐藏）
#   或一次性远程（会交互提示输密码）:
#     ssh -t mx93b dvt-unlock
#
# 安全：密码 echo 关闭读取，不进 argv / 不进 shell history / 不落盘 —— 直接写 tmpfs 600 并从
# 内存丢弃。密码错 → keystore 解密失败 → 本脚本检测到后清掉 tmpfs 里的错误密码并打印（掩敏感）日志。
set -eu

PASS_DIR=/run/dvt
PASS_FILE=$PASS_DIR/pass
# 端口从 dvt.env 读，不硬编码
PORT="$(grep -E '^PORT=' /opt/dvt-build/dvt.env 2>/dev/null | cut -d= -f2 | tr -d ' ')"
PORT="${PORT:-8080}"
HEALTH="http://127.0.0.1:$PORT/health"

# --- 已在跑就幂等退出 ---
if [ "$(systemctl is-active dvt 2>/dev/null)" = active ] \
   && curl -sf --max-time 4 "$HEALTH" >/dev/null 2>&1; then
  printf '✅ dvt3 已在运行:\n'; curl -s --max-time 4 "$HEALTH"; printf '\n'; exit 0
fi

# --- 隐藏读取密码（不进 argv/history/磁盘）---
printf 'Enter dvt3 keystore passphrase (DVT3_SECRET) — 输入隐藏: '
stty -echo 2>/dev/null || true
IFS= read -r SECRET
stty echo 2>/dev/null || true
printf '\n'
[ -n "${SECRET:-}" ] || { echo '空密码 — 取消'; exit 1; }

# --- 只写 tmpfs，600 ---
umask 077
mkdir -p "$PASS_DIR"; chmod 700 "$PASS_DIR"
printf 'NODE_KEY_PASSPHRASE=%s\n' "$SECRET" > "$PASS_FILE"
chmod 600 "$PASS_FILE"
SECRET=; unset SECRET   # 从 shell 内存丢弃明文

# --- 启动（非阻塞）+ 轮询 keystore-解密-门控的 /health ---
systemctl reset-failed dvt 2>/dev/null || true
systemctl --no-block start dvt

printf 'starting dvt3 (:%s)' "$PORT"
i=0
while [ "$i" -lt 20 ]; do
  if curl -sf --max-time 3 "$HEALTH" >/dev/null 2>&1; then
    printf '\n✅ dvt3 已解锁并运行（keystore 解密成功）:\n'; curl -s "$HEALTH"; printf '\n'; exit 0
  fi
  if [ "$(systemctl is-active dvt 2>/dev/null)" = failed ]; then
    printf '\n❌ dvt 启动失败 —— 密码可能错（keystore 解密失败）。\n'
    rm -f "$PASS_FILE"     # 清掉 tmpfs 里的错误密码
    printf '   tmpfs 密码已清除，用正确密码重跑 dvt-unlock。\n'
    printf '   --- 最近日志（掩敏感）---\n'
    journalctl -u dvt --no-pager -n 8 2>/dev/null \
      | sed -E 's/(passphrase|secret|password)[=: ]+\S+/\1=***/Ig'
    exit 1
  fi
  printf '.'; sleep 2; i=$((i + 1))
done
printf '\n⚠️  ~40s 后仍未健康。查: systemctl status dvt ; journalctl -u dvt -n 20\n'
exit 1
