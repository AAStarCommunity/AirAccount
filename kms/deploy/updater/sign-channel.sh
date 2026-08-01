#!/usr/bin/env bash
# 给 channel manifest 签名(CI 侧 / 测试共用)。
# 用法: sign-channel.sh <channel.json> [seckey]
#   seckey 缺省读 $MINISIGN_SECKEY;签名私钥应放 CI secret / 离线,不入库。
# 产出: <channel.json>.minisig
set -euo pipefail
CHANNEL="${1:?用法: sign-channel.sh <channel.json> [seckey]}"
SECKEY="${2:-${MINISIGN_SECKEY:-}}"
[ -f "$CHANNEL" ] || { echo "找不到 $CHANNEL" >&2; exit 1; }
command -v minisign >/dev/null || { echo "缺 minisign" >&2; exit 1; }
# 校验 JSON 合法
command -v jq >/dev/null && jq empty "$CHANNEL"

if [ -n "$SECKEY" ]; then
  # 非交互:私钥无密码(CI)或密码走 $MINISIGN_PASSWORD 由调用方 expect 处理;此处假设无密码。
  minisign -S -s "$SECKEY" -m "$CHANNEL" -x "$CHANNEL.minisig" -c "aastar node channel manifest"
else
  minisign -S -m "$CHANNEL" -x "$CHANNEL.minisig" -c "aastar node channel manifest"
fi
echo "signed → $CHANNEL.minisig"
