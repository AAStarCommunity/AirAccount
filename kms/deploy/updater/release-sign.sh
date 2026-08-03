#!/usr/bin/env bash
# release-sign.sh —— 签发一个社区节点 release:组装并签名 channel manifest(stable.json)。
#
# 发版链路:
#   1) 构建节点 tarball  airaccount-node-v<ver>.tar.gz(含 kms-api-server + TA + unit + manifest)
#   2) 【本脚本】算 sha256 → 写入/更新 channels/<channel>.json 的 releases[] → bump
#      metadata_version → 刷新 generated_at/expires → jq 校验 schema → minisign 私钥签名
#   3) 把 tarball 传到 GitHub release;把 <channel>.json + <channel>.json.minisig 传到
#      节点会拉的稳定 URL(updater 的 AU_MANIFEST_BASE)
#
# 用法:
#   release-sign.sh --version 0.30.0 --tarball dist/airaccount-node-v0.30.0.tar.gz \
#       [--severity high --security --notes "修 X"] [--ta-changed] [--dry-run]
#
# 关键选项(其余见下方 usage):
#   --ta-changed     标记本次含 TA 变更 —— 节点会**拒绝在线 apply**(决策 D),只能 OOB 刷。
#   --dry-run        只组装 + jq 校验,不签名(CI/测试用;不需要私钥)。
#
# 私钥默认 ~/.ssh/aastar-updater.key(密码加密;minisign 会交互提示输密码)。绝不入库。
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"

# ── 默认值 ────────────────────────────────────────────────────────────
CHANNEL="stable"
VERSION=""
TARBALL=""
TARBALL_URL=""
SEVERITY="none"
SECURITY="false"
TA_CHANGED="false"
AUTO_APPLY="true"
NOTES=""
NOTES_URL=""
MIN_VERSION=""
REQUIRES_TA=""
PROTO_VERSION=3
EXPIRES_DAYS=7
ROLLBACK_FLOOR=""
SECKEY="${MINISIGN_SECKEY:-$HOME/.ssh/aastar-updater.key}"
OUT=""
DRY_RUN=0
REPO="AAStarCommunity/AirAccount"

usage() { sed -n '2,30p' "$0"; exit "${1:-0}"; }

while [ "$#" -gt 0 ]; do
  case "$1" in
    --channel)        CHANNEL="$2"; shift 2 ;;
    --version)        VERSION="$2"; shift 2 ;;
    --tarball)        TARBALL="$2"; shift 2 ;;
    --tarball-url)    TARBALL_URL="$2"; shift 2 ;;
    --severity)       SEVERITY="$2"; shift 2 ;;
    --security)       SECURITY="true"; shift ;;
    --ta-changed)     TA_CHANGED="true"; shift ;;
    --no-auto-apply)  AUTO_APPLY="false"; shift ;;
    --notes)          NOTES="$2"; shift 2 ;;
    --notes-url)      NOTES_URL="$2"; shift 2 ;;
    --min-version)    MIN_VERSION="$2"; shift 2 ;;
    --requires-ta)    REQUIRES_TA="$2"; shift 2 ;;
    --proto-version)  PROTO_VERSION="$2"; shift 2 ;;
    --expires-days)   EXPIRES_DAYS="$2"; shift 2 ;;
    --rollback-floor) ROLLBACK_FLOOR="$2"; shift 2 ;;
    --seckey)         SECKEY="$2"; shift 2 ;;
    --out)            OUT="$2"; shift 2 ;;
    --dry-run)        DRY_RUN=1; shift ;;
    -h|--help)        usage 0 ;;
    *) echo "未知参数: $1" >&2; usage 1 ;;
  esac
done

command -v jq >/dev/null || { echo "缺 jq" >&2; exit 1; }
[ -n "$VERSION" ] || { echo "必须 --version x.y.z" >&2; usage 1; }
echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "版本号需 x.y.z(不带 v 前缀)" >&2; exit 1; }
echo "$SEVERITY" | grep -qE '^(none|low|medium|high|critical)$' || { echo "severity 非法" >&2; exit 1; }

OUT="${OUT:-$HERE/channels/$CHANNEL.json}"
TARBALL_URL="${TARBALL_URL:-https://github.com/$REPO/releases/download/airaccount-node-v$VERSION/airaccount-node-v$VERSION.tar.gz}"
NOTES_URL="${NOTES_URL:-https://github.com/$REPO/releases/tag/airaccount-node-v$VERSION}"

# ── sha256(需 tarball 或显式传 --sha256 未来可加)────────────────────
[ -n "$TARBALL" ] || { echo "必须 --tarball <path>(用于算 sha256)" >&2; exit 1; }
[ -f "$TARBALL" ] || { echo "找不到 tarball: $TARBALL" >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then SHA="$(sha256sum "$TARBALL" | awk '{print $1}')";
else SHA="$(shasum -a 256 "$TARBALL" | awk '{print $1}')"; fi
echo "$SHA" | grep -qE '^[0-9a-fA-F]{64}$' || { echo "sha256 计算异常" >&2; exit 1; }

# ── 继承旧 manifest(metadata_version 单调、releases 累积)──────────────
PREV_META=0
PREV_RELEASES='[]'
if [ -f "$OUT" ] && jq empty "$OUT" 2>/dev/null; then
  PREV_META="$(jq -r '.metadata_version // 0' "$OUT")"
  PREV_RELEASES="$(jq -c '.releases // []' "$OUT")"
fi
NEW_META=$((PREV_META + 1))

# 缺省 min_version / requires_ta / rollback_floor:尽量从上一条最高 release 继承,否则用保守值。
if [ -z "$MIN_VERSION" ]; then
  MIN_VERSION="$(echo "$PREV_RELEASES" | jq -r 'map(.min_version) | (.[0] // "0.28.0")')"
fi
if [ -z "$REQUIRES_TA" ]; then
  REQUIRES_TA="$(echo "$PREV_RELEASES" | jq -r 'map(.requires_ta_version) | (.[0] // "0.28.0")')"
fi
if [ -z "$ROLLBACK_FLOOR" ]; then
  ROLLBACK_FLOOR="$(jq -r '.rollback_floor // "0.28.0"' "$OUT" 2>/dev/null || echo "0.28.0")"
fi

# 跨 mac/linux 的 UTC 时间戳
NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if date -u -d "@0" >/dev/null 2>&1; then
  EXPIRES="$(date -u -d "+$EXPIRES_DAYS days" +%Y-%m-%dT%H:%M:%SZ)"      # GNU
else
  EXPIRES="$(date -u -v +"${EXPIRES_DAYS}"d +%Y-%m-%dT%H:%M:%SZ)"        # BSD/mac
fi

# ── 组装 release 条目 + 合并(同版本则替换,否则前插)+ 顶层字段 ─────────
NEW_RELEASE="$(jq -n \
  --arg version "$VERSION" --argjson security "$SECURITY" --arg severity "$SEVERITY" \
  --arg notes "$NOTES" --arg notes_url "$NOTES_URL" --argjson auto "$AUTO_APPLY" \
  --argjson ta_changed "$TA_CHANGED" --arg min "$MIN_VERSION" --arg reqta "$REQUIRES_TA" \
  --argjson proto "$PROTO_VERSION" --arg tarball "$TARBALL_URL" --arg sha "$SHA" '
  {version:$version, security:$security, severity:$severity, notes:$notes, notes_url:$notes_url,
   auto_apply_allowed:$auto, ta_changed:$ta_changed, min_version:$min,
   requires_ta_version:$reqta, proto_version:$proto, tarball:$tarball, sha256:$sha, canary_ring:[]}')"

MANIFEST="$(jq -n \
  --argjson meta "$NEW_META" --arg gen "$NOW" --arg exp "$EXPIRES" --arg channel "$CHANNEL" \
  --arg floor "$ROLLBACK_FLOOR" --argjson prev "$PREV_RELEASES" --argjson new "$NEW_RELEASE" '
  {metadata_version:$meta, generated_at:$gen, expires:$exp, channel:$channel, rollback_floor:$floor,
   releases: ([$new] + ($prev | map(select(.version != $new.version))))}')"

# ── schema 自检(对齐 updater load_manifest 的关键校验,fail-closed)────
echo "$MANIFEST" | jq -e '
  (.metadata_version|type=="number")
  and (.expires|type=="string")
  and (.releases|type=="array" and length>0)
  and (all(.releases[];
        (.version|test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$"))
        and (.sha256|test("^[0-9a-fA-F]{64}$"))
        and ((.ta_changed)|type=="boolean")
        and (.severity|test("^(none|low|medium|high|critical)$"))
        and (.min_version|test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$"))
        and (.requires_ta_version==null or (.requires_ta_version|test("^v?[0-9]+\\.[0-9]+\\.[0-9]+$")))
      ))
' >/dev/null || { echo "组装出的 manifest 未过 schema 自检" >&2; echo "$MANIFEST" | jq . >&2; exit 1; }

mkdir -p "$(dirname "$OUT")"
echo "$MANIFEST" | jq . > "$OUT"
echo "✅ manifest 已写: $OUT"
echo "   version=$VERSION  metadata_version=${PREV_META}->${NEW_META}  sha256=$SHA"
echo "   severity=$SEVERITY security=$SECURITY ta_changed=$TA_CHANGED expires=$EXPIRES"
[ "$TA_CHANGED" = "true" ] && echo "   ⚠️ ta_changed=true:节点将拒绝在线 apply,只能 OOB 刷 TA。"

if [ "$DRY_RUN" = 1 ]; then
  echo "（--dry-run:跳过签名。校验签发结果:上传 $OUT + .minisig 到 AU_MANIFEST_BASE，tarball 到 release）"
  exit 0
fi

# ── 签名(minisign;私钥密码交互输入)──────────────────────────────────
command -v minisign >/dev/null || { echo "缺 minisign" >&2; exit 1; }
[ -f "$SECKEY" ] || { echo "找不到私钥 $SECKEY(--seckey 指定,或放 ~/.ssh/aastar-updater.key)" >&2; exit 1; }
echo "→ 用 $SECKEY 签名(将提示输入私钥密码)…"
minisign -S -s "$SECKEY" -m "$OUT" -x "$OUT.minisig" -c "aastar node channel manifest ($CHANNEL v$VERSION)"
echo "✅ 签名已写: $OUT.minisig"

# ── 立即自验(用仓库内公钥)──────────────────────────────────────────
PUB="$HERE/updater-pubkey.pub"
if [ -f "$PUB" ]; then
  minisign -V -p "$PUB" -m "$OUT" -x "$OUT.minisig" >/dev/null \
    && echo "✅ 用 $PUB 自验通过" \
    || { echo "❌ 自验失败!签名与仓库公钥不匹配 —— 私钥是否用错?" >&2; exit 1; }
fi

cat <<EOF

下一步(发布):
  1. 把 tarball 传到 release:
       gh release create airaccount-node-v$VERSION "$TARBALL" -t "airaccount-node v$VERSION" -n "$NOTES"
     (或 gh release upload airaccount-node-v$VERSION "$TARBALL")
  2. 把签名 manifest 传到节点会拉的 URL(AU_MANIFEST_BASE):
       $OUT
       $OUT.minisig
EOF
