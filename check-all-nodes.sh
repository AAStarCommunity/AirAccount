#!/usr/bin/env bash
# check-all-nodes.sh — 一键只读健康巡检
#   覆盖：imx93 A 板 / B 板 + DK2，DVT1/2/3 (2-of-3 门限)，公网 KMS 端点。
#   纯只读：只 curl /health、systemctl is-active，绝不改任何状态。
#
# 拓扑（kms/docs/community-node-topology.md）：
#   DVT1 = A 板 mx93   (KMS-TEE 托管, :8080)   —— 机房，公网 kms.aastar.io
#   DVT2 = DK2  dk2-kms (本地 keystore, :8080)  —— 机房 armv7，ProxyJump mac-mini
#   DVT3 = B 板 mx93b  (本地 keystore, :8080)   —— 家，公网 kms1.aastar.io
#   → DVT1/2/3 组 2-of-3 门限（容忍 1 挂）。
#
# 在哪跑最好：Mac mini（Tailscale + DK2 同 LAN 直连，无需 jump）。见文末说明。
# 用法：  bash check-all-nodes.sh
set -uo pipefail

SSH_OPTS=(-o ConnectTimeout=10 -o BatchMode=yes)
G=$'\033[32m'; R=$'\033[31m'; Y=$'\033[33m'; DIM=$'\033[2m'; N=$'\033[0m'
ok(){ printf "${G}✅ %s${N}\n" "$1"; }
bad(){ printf "${R}❌ %s${N}\n" "$1"; }
warn(){ printf "${Y}⚠️  %s${N}\n" "$1"; }

# ---- 板子: 标签 | ssh-host | 公网 health url(可空) ----
BOARDS=(
  "A 板 (机房)|mx93|https://kms.aastar.io/health"
  "B 板 (家)|mx93b|https://kms1.aastar.io/health"
  "DK2 (机房)|dk2-kms|"
)
# ---- DVT: 标签 | ssh-host ----  (统一 127.0.0.1:8080/health)
DVTS=(
  "DVT1|mx93"
  "DVT2|dk2-kms"
  "DVT3|mx93b"
)

dvt_ok=0; dvt_total=0

echo "════════════════════════════════════════════════════════"
echo " AirAccount 节点巡检  ·  $(date '+%Y-%m-%d %H:%M:%S %Z')"
echo "════════════════════════════════════════════════════════"

# ---------- 1. 板子 (SSH + 服务 + 公网端点) ----------
echo ""
echo "── 板子 (SSH / systemd 服务 / 公网端点) ──"
for row in "${BOARDS[@]}"; do
  IFS='|' read -r label host url <<<"$row"
  printf "\n▸ %s  ${DIM}[%s]${N}\n" "$label" "$host"

  svc=$(ssh "${SSH_OPTS[@]}" "$host" \
        'echo "$(systemctl is-active kms-api 2>/dev/null||echo n/a)|$(systemctl is-active dvt 2>/dev/null)|$(systemctl is-active cloudflared 2>/dev/null||echo n/a)|$(uptime|sed s/.*up/up/|cut -d, -f1)"' 2>/dev/null)
  if [ -z "$svc" ]; then
    bad "SSH 不通 —— 板子未上电 / 不可达"
  else
    IFS='|' read -r kms dvt cf up <<<"$svc"
    ok "SSH 在线  ${DIM}(${up# })${N}"
    for pair in "kms-api:$kms" "dvt:$dvt" "cloudflared:$cf"; do
      name=${pair%%:*}; st=${pair#*:}
      case "$st" in
        active) printf "     ${G}%-12s active${N}\n" "$name" ;;
        n/a)    printf "     ${DIM}%-12s n/a${N}\n" "$name" ;;
        *)      printf "     ${R}%-12s %s${N}\n" "$name" "$st" ;;
      esac
    done
  fi

  if [ -n "$url" ]; then
    code=$(curl -sS -m 10 -o /dev/null -w "%{http_code}" "$url" 2>/dev/null)
    t=$(curl -sS -m 10 -o /dev/null -w "%{time_total}" "$url" 2>/dev/null)
    if [ "$code" = "200" ]; then printf "     ${G}%-12s HTTP 200 (%ss)${N}\n" "public" "$t"
    else printf "     ${R}%-12s HTTP %s${N}  %s\n" "public" "${code:-000}" "$url"; fi
  fi
done

# ---------- 2. DVT 门限 (2-of-3) ----------
echo ""
echo "── DVT 节点 (127.0.0.1:8080/health · 2-of-3 门限) ──"
for row in "${DVTS[@]}"; do
  IFS='|' read -r label host <<<"$row"
  dvt_total=$((dvt_total+1))
  body=$(ssh "${SSH_OPTS[@]}" "$host" \
         'curl -sS -m3 http://127.0.0.1:8080/health 2>/dev/null' 2>/dev/null)
  if echo "$body" | grep -q '"status":"ok"'; then
    ver=$(echo "$body" | grep -oE '"version":"[^"]*"' | head -1 | cut -d'"' -f4)
    opsalert=$(echo "$body" | grep -oE '"name":"ops-alert","enabled":(true|false)' | grep -oE '(true|false)$')
    dvt_ok=$((dvt_ok+1))
    printf "  ${G}✅ %-6s${N} v%-8s ${DIM}[%s]  ops-alert=%s${N}\n" "$label" "${ver:-?}" "$host" "${opsalert:-?}"
  elif [ -z "$body" ]; then
    printf "  ${R}❌ %-6s${N} 不可达 (SSH 或 :8080 无响应) ${DIM}[%s]${N}\n" "$label" "$host"
  else
    printf "  ${R}❌ %-6s${N} 异常: %s ${DIM}[%s]${N}\n" "$label" "$(echo "$body"|head -c 60)" "$host"
  fi
done

# ---------- 3. 门限裁决 ----------
echo ""
echo "════════════════════════════════════════════════════════"
if [ "$dvt_ok" -ge 2 ]; then
  ok "DVT 门限满足：$dvt_ok/$dvt_total 在线 (≥2)，验证器可正常出块"
else
  bad "DVT 门限告破：$dvt_ok/$dvt_total 在线 (<2)，2-of-3 无法签名！"
fi
echo "════════════════════════════════════════════════════════"

# 退出码：门限破 = 2；有任一 DVT 挂但门限仍满足 = 1；全绿 = 0
if [ "$dvt_ok" -lt 2 ]; then exit 2
elif [ "$dvt_ok" -lt "$dvt_total" ]; then exit 1
else exit 0; fi
