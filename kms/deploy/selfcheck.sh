#!/usr/bin/env bash
# selfcheck.sh — AirAccount 全网快速自检(只读,不改任何状态)
#   板A(机房 mx93a) + 板B(家 mx93b) + DK2(机房 dvt2) · DVT 2-of-3 门限 · 公网 KMS
# 用法:  bash kms/deploy/selfcheck.sh
# 依赖:  ssh(Tailscale 可达) + curl。DK2 走 Tailscale IP;板B 走 ssh 别名 mx93b。
set -uo pipefail
SO=(-o StrictHostKeyChecking=no -o ConnectTimeout=10 -o BatchMode=yes)
G=$'\033[32m'; R=$'\033[31m'; Y=$'\033[33m'; D=$'\033[2m'; N=$'\033[0m'

# 标签 | ssh目标 | 公网URL(可空) | dvt单元名
NODES=(
  "板A 机房 (dvt1 · KMS-TEE)|root@100.121.187.3|https://kms.aastar.io|dvt"
  "板B 家   (dvt3 · keystore)|mx93b|https://kms1.aastar.io|dvt"
  "DK2 机房 (dvt2 · keystore)|root@100.78.134.114||aastar-dvt@node2"
)

echo "════════════════════════════════════════════════════════"
echo " AirAccount 自检  ·  $(date '+%F %T %Z')"
echo "════════════════════════════════════════════════════════"

dvt_ok=0
for row in "${NODES[@]}"; do
  IFS='|' read -r label host url dvtunit <<<"$row"
  printf "\n▸ %s  ${D}[%s]${N}\n" "$label" "$host"

  info=$(ssh "${SO[@]}" "$host" "
    printf '%s|%s|%s|%s' \
      \"\$(systemctl is-active kms-api 2>/dev/null || echo n/a)\" \
      \"\$(systemctl is-active $dvtunit 2>/dev/null || echo n/a)\" \
      \"\$(systemctl is-active cloudflared 2>/dev/null || echo n/a)\" \
      \"\$(uptime 2>/dev/null | sed 's/.*up/up/;s/,.*load/ (load/')\" " 2>/dev/null)

  if [ -z "$info" ]; then
    printf "  ${R}❌ SSH 不通 —— 未上电 / 网络未恢复 / 还在开机${N}\n"
  else
    IFS='|' read -r kms dvt cf up <<<"$info"
    printf "  ${G}✅ SSH 在线${N}  ${D}%s${N}\n" "$up"
    for pair in "kms-api:$kms" "dvt:$dvt" "cloudflared:$cf"; do
      nm=${pair%%:*}; st=${pair#*:}
      case "$st" in
        active) printf "     ${G}%-12s active${N}\n" "$nm" ;;
        n/a)    printf "     ${D}%-12s n/a${N}\n"    "$nm" ;;
        *)      printf "     ${R}%-12s %s${N}\n"     "$nm" "$st" ;;
      esac
    done
    [ "$dvt" = active ] && dvt_ok=$((dvt_ok+1))
  fi

  if [ -n "$url" ]; then
    read -r code t < <(curl -sS -m10 -o /dev/null -w "%{http_code} %{time_total}" "$url/health" 2>/dev/null || echo "000 -")
    if [ "$code" = 200 ]; then printf "     ${G}%-12s HTTP 200 (%ss)${N}\n" "公网" "$t"
    else printf "     ${R}%-12s HTTP %s${N}  %s\n" "公网" "$code" "$url"; fi
  fi
done

echo ""
echo "── DVT 门限 (2-of-3) ──"
if [ "$dvt_ok" -ge 2 ]; then
  printf "  ${G}✅ %d/3 在线 —— 门限满足,验证器正常${N}\n" "$dvt_ok"
else
  printf "  ${R}❌ 仅 %d/3 在线 —— 门限不满足!${N}\n" "$dvt_ok"
fi
echo "════════════════════════════════════════════════════════"
