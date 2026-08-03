#!/usr/bin/env bash
# updater 通知 hook → Telegram(AAStarMonitorBot)。
# 作为 AU_NOTIFY_CMD 注入(见 updater.env),被 updater 以 `<script> <level> <msg>` 调用。
#
#   updater.env:  AU_NOTIFY_CMD="/opt/airaccount/updater/notify-telegram.sh"
#   凭据(同 updater.env 或环境):TELEGRAM_BOT_TOKEN(=AAstarMonitorBot_TOKEN)、TELEGRAM_CHAT_ID
#
# 退出码约定(updater 的去重逻辑依赖,见 notify_update):
#   0 = 已送达 或 未配置凭据(无处可送,视为无需重推,不会永久重试)
#   非0(3)= 已配置但发送失败(网络/HTTP 错)—— updater 不阻断,但**不写去重 key**,下次重推
# 说明:updater 的 notify() 对本 hook 加了 `|| true`,故非零退出不会打断主流程;
#       只有走「送达状态」判定的 notify_send/notify_update 才在意这个码。
#
# 安全:
#   - bot token 在 URL 里,故 URL 走 `curl --config -`(stdin)传入,**不出现在 argv/ps**(codex M5)。
#   - 纯文本发送(不用 MarkdownV2),消息体由 updater 组装,避免特殊字符转义踩坑。
#   - CURL 可注入(测试):CURL 环境变量覆盖 curl 可执行。
#   - 静默(codex L1):诊断信息仅在 TELEGRAM_DEBUG=1 时输出到 stderr。
set -uo pipefail

LEVEL="${1:-info}"
MSG="${2:-}"

TOKEN="${TELEGRAM_BOT_TOKEN:-${AAstarMonitorBot_TOKEN:-}}"
CHAT="${TELEGRAM_CHAT_ID:-}"
CURL="${CURL:-curl}"

dbg() { [ -n "${TELEGRAM_DEBUG:-}" ] && echo "[telegram] $*" >&2 || true; }

if [ -z "$TOKEN" ] || [ -z "$CHAT" ]; then
  dbg "未配置 TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID —— 跳过(exit 0)"
  exit 0
fi

case "$LEVEL" in
  error) emoji="🛑" ;;
  warn)  emoji="⚠️" ;;
  *)     emoji="🔔" ;;
esac
TEXT="${emoji} [AAStar 节点更新] ${MSG}"
# Telegram sendMessage 硬上限 4096 字符;超长会被 API 拒(→ 本 hook 非零 → updater 不写
# 去重 key → 每个 timer 周期永远重试失败,pr-daemon #5)。这里防御性截断,保证能送达。
# (上游 manifest schema 也已限 notes≤280,双保险。)
if [ "${#TEXT}" -gt 3900 ]; then TEXT="${TEXT:0:3900}…(截断)"; fi

# URL(含 token)走 --config stdin,不进 argv;data 在 argv(多行文本 argv 安全)。
if printf 'url = "https://api.telegram.org/bot%s/sendMessage"\n' "$TOKEN" \
   | "$CURL" -fsS --max-time 15 -X POST --config - \
       --data-urlencode "chat_id=${CHAT}" \
       --data-urlencode "text=${TEXT}" \
       --data-urlencode "disable_web_page_preview=true" \
       >/dev/null 2>&1; then
  exit 0
else
  dbg "发送失败(网络/HTTP?)—— 返回 3(updater 不阻断,但不写去重 key)"
  exit 3
fi
