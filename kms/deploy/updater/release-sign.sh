#!/usr/bin/env bash
# release-sign.sh —— 签发一个社区节点 release:组装并签名 channel manifest(stable.json)。
#
# 发版链路:
#   1) 构建节点 tarball  airaccount-node-v<ver>.tar.gz(含 kms-api-server + TA + unit + manifest)
#   2) 【本脚本】算 sha256 → 读回已发布 manifest 取 metadata_version 基线 → 组装/累积 releases[]
#      → **单调** bump metadata_version → 刷新 generated_at/expires → schema 自检(镜像节点
#      load_manifest)→ 写临时文件 → minisign 签 → 用仓库公钥自验 → **原子替换** 正文+签名
#   3) 把 tarball 传到 GitHub release;把 <channel>.json + <channel>.json.minisig 传到
#      节点会拉的稳定 URL(updater 的 AU_MANIFEST_BASE)
#
# 用法:
#   release-sign.sh --version 0.30.0 --tarball dist/airaccount-node-v0.30.0.tar.gz \
#       [--severity high --security --notes "修 X"] [--ta-changed] [--dry-run]
#
# 关键选项(其余见下方 usage):
#   --ta-changed     标记本次含 TA 变更 —— 节点会**拒绝在线 apply**(决策 D),只能 OOB 刷。
#   --dry-run        只组装 + schema 自检,打到 stdout,**绝不触碰 $OUT / 不签名**(不需私钥)。
#   --base-url URL   读回已发布 manifest 求 metadata_version 基线(默认 AU_MANIFEST_BASE)。
#   --no-baseline    跳过读回(离线/首发;此时 metadata_version 仅从本地 $OUT 取,慎用)。
#   --revoke VER[,VER…]  撤销版本:写进单调 revoked[] 墓碑,并从 releases 剔除。撤销的唯一机制
#                    (不是"删 releases 条目")。被撤销的版本不能再 --version 重签。可重复/逗号列表。
#                    可单独用(不带 --version)做**纯撤销**;撤销最后一版会得到 releases:[](合法)。
#   --trust-unsigned-local  本地 $OUT 无 .minisig 时显式信任(首发/迁移)。默认拒:未签名本地 $OUT
#                    无界驱动 metadata_version/floor/revoked(全单调)→ 改一下就能**永久砖化全网**。
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
PUBKEY_FILE=""   # 默认 $HERE/updater-pubkey.pub;--pubkey 覆盖(测试/换轮公钥用)
OUT=""
DRY_RUN=0
BASE_URL="${AU_MANIFEST_BASE:-https://raw.githubusercontent.com/AAStarCommunity/AirAccount/main/kms/deploy/updater/channels}"
NO_BASELINE=0
REVOKE_LIST=""          # --revoke <ver>[,...] 累积:写进 revoked[] 墓碑,从 releases 剔除(#196 R6 撤销机制)
TRUST_UNSIGNED_LOCAL=0  # 本地 $OUT 无 .minisig 时是否信任(默认拒:未签名本地无界驱动 counter=永久砖化,#196 R8)
REPO="AAStarCommunity/AirAccount"
NOTES_MAX=280   # 节点 load_manifest 对 notes 的硬上限(与之保持一致)

die() { echo "release-sign: $*" >&2; exit 1; }
usage() { sed -n '2,30p' "$0"; exit "${1:-0}"; }
# 统一清理:所有临时路径(读回目录 + 签名临时文件)进此数组,一个 trap 收。
CLEANUP_PATHS=()
cleanup() { [ "${#CLEANUP_PATHS[@]}" -gt 0 ] && rm -rf "${CLEANUP_PATHS[@]}" 2>/dev/null || true; }
trap cleanup EXIT

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
    --pubkey)         PUBKEY_FILE="$2"; shift 2 ;;
    --out)            OUT="$2"; shift 2 ;;
    --base-url)       BASE_URL="$2"; shift 2 ;;
    --no-baseline)    NO_BASELINE=1; shift ;;
    --revoke)         REVOKE_LIST="${REVOKE_LIST:+$REVOKE_LIST,}$2"; shift 2 ;;   # 撤销版本 → revoked[] 墓碑
    --trust-unsigned-local) TRUST_UNSIGNED_LOCAL=1; shift ;;   # 显式信任无 .minisig 的本地 $OUT(唯一存活用途=配 --no-baseline;有基线时仍拒绝其抬高不可逆字段)
    --dry-run)        DRY_RUN=1; shift ;;
    -h|--help)        usage 0 ;;
    *) echo "未知参数: $1" >&2; usage 1 ;;
  esac
done

# ── 前置校验(全部在**触碰 $OUT 之前**做;非 dry-run 连工具/私钥/公钥都先查齐)──────
command -v jq >/dev/null || die "缺 jq"
[ -n "$VERSION" ] || [ -n "$REVOKE_LIST" ] || { echo "必须 --version x.y.z(或仅 --revoke 做**纯撤销**:不发新版本,只把版本写进 revoked 并从 releases 剔除)" >&2; usage 1; }
[ -z "$VERSION" ] || [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "版本号需 x.y.z(不带 v 前缀): $VERSION"
[[ "$SEVERITY" =~ ^(none|low|medium|high|critical)$ ]] || die "severity 非法: $SEVERITY"
# --channel:只允许字母数字/点/横杠/下划线,防越目录写(../../tmp/pwn)。
[[ "$CHANNEL" =~ ^[A-Za-z0-9._-]+$ ]] || die "channel 名非法(只允许 [A-Za-z0-9._-]): $CHANNEL"
[[ "$EXPIRES_DAYS" =~ ^[0-9]+$ ]] && [ "$EXPIRES_DAYS" -ge 1 ] || die "--expires-days 需正整数: $EXPIRES_DAYS"
[[ "$PROTO_VERSION" =~ ^[0-9]+$ ]] || die "--proto-version 需整数: $PROTO_VERSION"
[ -z "$MIN_VERSION" ] || [[ "$MIN_VERSION" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "--min-version 非法: $MIN_VERSION"
[ -z "$REQUIRES_TA" ] || [[ "$REQUIRES_TA" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "--requires-ta 非法: $REQUIRES_TA"
[ -z "$ROLLBACK_FLOOR" ] || [[ "$ROLLBACK_FLOOR" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "--rollback-floor 非法: $ROLLBACK_FLOOR"
# --revoke 每个值都要是合法 semver(逗号分隔)
if [ -n "$REVOKE_LIST" ]; then
  IFS=',' read -ra _rv <<< "$REVOKE_LIST"
  for _v in "${_rv[@]}"; do [[ "$_v" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "--revoke 版本非法(需 x.y.z): $_v"; done
fi
# --notes:节点要求无控制字符且 ≤NOTES_MAX;违反会被**每个节点** fail-closed 拒绝 → 在此就拒。
if [ -n "$NOTES" ]; then
  # -z:把整个 NOTES 当**一条** NUL 记录扫,否则 grep 会把换行当行分隔符、扫不到内嵌换行。
  printf '%s' "$NOTES" | LC_ALL=C grep -zq '[[:cntrl:]]' && die "--notes 含控制字符(换行/制表等),节点会拒绝该 manifest"
  [ "${#NOTES}" -le "$NOTES_MAX" ] || die "--notes 超 $NOTES_MAX 字符(${#NOTES}),节点会拒绝"
fi

OUT="${OUT:-$HERE/channels/$CHANNEL.json}"
TARBALL_URL="${TARBALL_URL:-https://github.com/$REPO/releases/download/airaccount-node-v$VERSION/airaccount-node-v$VERSION.tar.gz}"
NOTES_URL="${NOTES_URL:-https://github.com/$REPO/releases/tag/airaccount-node-v$VERSION}"

# 非 dry-run:签名工具链 + 私钥 + 自验公钥必须**先**齐备(否则别动 $OUT)。
if [ "$DRY_RUN" != 1 ]; then
  command -v minisign >/dev/null || die "缺 minisign"
  [ -f "$SECKEY" ] || die "找不到私钥 $SECKEY(--seckey 指定,或放 ~/.ssh/aastar-updater.key)"
  PUB="${PUBKEY_FILE:-$HERE/updater-pubkey.pub}"
  [ -f "$PUB" ] || die "缺自验公钥 $PUB —— 签发环节必须能自验,拒绝跳过"
fi

# ── sha256 ────────────────────────────────────────────────────────────
SHA=""   # 纯撤销(仅 --revoke,无 --version)不发新版本 → 不算 sha;显式初始化免 set -u
if [ -n "$VERSION" ]; then
  [ -n "$TARBALL" ] || die "必须 --tarball <path>(用于算 sha256)"
  [ -f "$TARBALL" ] || die "找不到 tarball: $TARBALL"
  if command -v sha256sum >/dev/null 2>&1; then SHA="$(sha256sum "$TARBALL" | awk '{print $1}')";
  else SHA="$(shasum -a 256 "$TARBALL" | awk '{print $1}')"; fi
  [[ "$SHA" =~ ^[0-9a-fA-F]{64}$ ]] || die "sha256 计算异常"
fi

# semver_max a b → 打印较大者(去 v 前缀,3 段数字比较;空/缺参当 0.0.0)。用于 floor 反回退。
semver_max() {
  jq -rn --arg a "${1:-0.0.0}" --arg b "${2:-0.0.0}" '
    def key: (ltrimstr("v")|split(".")|map(tonumber? // 0));
    if ($a|key) >= ($b|key) then $a else $b end'
}

# ── 基线:counter + releases[] + rollback_floor 都从**已验签的已发布 manifest** 继承 ──────
# 防回滚计数器不能只靠本地未入库文件(换机/新 clone → 本地 0 重来 → 节点 seen_metadata_version
# 不降 → 永久拒绝)。但读回的 manifest 是**不可信输入**:若只读它的 metadata_version 而不验签,
# 攻击者能喂一个 metadata_version=9e9 的假 manifest,让本脚本用**真私钥签**出一个天文计数 →
# 节点 ratchet 后永久拒绝一切后续版本(把本工具要防的砖化搬到了上游)。故:**先验签,后继承**,
# 且 counter/releases/floor **全部**从验签副本取(只取 counter 会在新 clone 上丢掉所有旧 release +
# 把 rollback_floor 悄悄降回 0.28.0)。读回失败(网络/端点)一律 fail-closed,首发须显式 --no-baseline。
# (#196 R10)metadata_version 上界:**读回门与签发门必须同一个常量**,否则出现「签发端产出自己读不回去
# 的 counter」(旧 bug:读门 1e9、签门 1e9+1 → 本地 1e9 签出 1e9+1,自己读回即 rc=1;节点 ratchet 到
# 1e9+1 后再签不出更高的 → 永久全网冻结)。不变量:**签出的 NEW_META 必须仍落在下次能读回的 [0,META_MAX]
# 内**。到顶 = counter 空间耗尽 → die(不抬门对齐、不静默饱和停在 MAX —— 后者会让两份不同内容共享同一
# counter,而节点只拒 < 不拒 =,是安全故障)。1e9 足够每天签发几十年。
META_MAX=1000000000
LOCAL_META=0; LOCAL_RELEASES='[]'; LOCAL_FLOOR=""; LOCAL_REVOKED='[]'; LOCAL_VERIFIED=0
if [ -f "$OUT" ] && jq empty "$OUT" 2>/dev/null; then
  # ⚠️ 本地 $OUT 无界驱动 metadata_version/rollback_floor/revoked(全**单调只增**),被改一下就能**永久
  # 砖化全网**(节点 ratchet 后 counter/floor/revoked 再也压不回来,持钥人自己也回不去)。默认 $OUT 在
  # **仓库工作树**、且收尾建议入库 → "能写 $OUT"的主体(仓库/CI merge 权限)严格大于持钥人。故信任本地
  # 4 个 LOCAL_* 之前,先验签**同目录的 $OUT.minisig**(签名就在旁边、零成本)—— 与读回基线同一道
  # "先验签后继承"(#196 R8 finding1/2:那道防线之前只加在读回路径,本地路径一道都没有)。
  # (#196 R9)把"已验证"提成一等变量 LOCAL_VERIFIED:验签≠验身份,还要 channel 匹配(B2);下方不可逆
  # 字段只认这个变量,不认"文件在哪"。R8 只判"文件签没签"、把三个不可逆字段的注入通道原样留着,正是漏洞。
  PUB_LOCAL="${PUBKEY_FILE:-$HERE/updater-pubkey.pub}"
  if [ -f "$OUT.minisig" ]; then
    command -v minisign >/dev/null || die "验本地 $OUT 需 minisign"
    [ -f "$PUB_LOCAL" ] || die "验本地 $OUT 缺公钥 $PUB_LOCAL(或删除 $OUT 首发)"
    minisign -V -p "$PUB_LOCAL" -m "$OUT" -x "$OUT.minisig" >/dev/null 2>&1 \
      || die "本地 $OUT 验签失败(疑篡改)—— 拒绝把未验证的 counter/floor/revoked/releases 洗进真钥签名产物"
    # (#196 R9 B2) 签名保真不保来源:一份**合法签名**的 beta.json 被(误)放到 stable 路径,不需要任何
    # flag 就能整份洗进 stable(低 floor 冻结全网 + 好版本被墓碑 + 预发布 releases 混入)。channel 必须
    # 匹配,宽严与读回路径 :203 逐字对齐(-z 承重:手写首发种子文件合法地无 .channel 键)。不搬 expires
    # 新鲜度(单调字段无法缩水,拦它只会打断"放了一周的工作树里签发")。
    LOCAL_CHANNEL="$(jq -r '.channel // empty' "$OUT")"
    [ -z "$LOCAL_CHANNEL" ] || [ "$LOCAL_CHANNEL" = "$CHANNEL" ] \
      || die "本地 $OUT 的 channel=$LOCAL_CHANNEL != 目标 $CHANNEL —— 拒绝(签名保真不保来源;疑 $LOCAL_CHANNEL 被喂到 $CHANNEL 的路径)"
    LOCAL_VERIFIED=1   # 验签 + channel 双过 → 唯一可信来源;下方 max/union/releases 只信这个变量
  elif [ "$TRUST_UNSIGNED_LOCAL" = 1 ]; then
    echo "⚠️ --trust-unsigned-local:本地 $OUT 无 .minisig,按显式授权信任(唯一存活用途=配 --no-baseline 首发/迁移;有基线时仍拒绝其抬高任一不可逆字段)" >&2
  else
    die "本地 $OUT 无 .minisig —— 拒绝静默信任其 counter/floor/revoked/releases(未签名本地无界驱动 counter=永久砖化;用 --trust-unsigned-local 显式信任,或删除 $OUT 走首发)"
  fi
  LOCAL_META="$(jq -r '(.metadata_version // 0) | floor' "$OUT")"
  # (#196 R9 F2 加固)未验证本地的 counter 也可能是 2^63-1 溢出 → 数字且有上界,否则 die(镜像节点)。
  [[ "$LOCAL_META" =~ ^[0-9]+$ ]] && [ "$LOCAL_META" -le "$META_MAX" ] \
    || die "本地 $OUT metadata_version=$LOCAL_META 非法(需 0..$META_MAX 整数)—— 拒绝(疑溢出/注入)"
  LOCAL_RELEASES="$(jq -c '.releases // []' "$OUT")"
  LOCAL_FLOOR="$(jq -r '.rollback_floor // empty' "$OUT")"
  LOCAL_REVOKED="$(jq -c '.revoked // []' "$OUT")"
fi
PREV_META="$LOCAL_META"; PREV_RELEASES="$LOCAL_RELEASES"; BASE_FLOOR="$LOCAL_FLOOR"; BASE_REVOKED="$LOCAL_REVOKED"
BASE_HAS_REVOKED=false   # 已验签基线**是否有** revoked 字段(缺字段=墓碑之前发布,迁移判别用)
if [ "$NO_BASELINE" != 1 ]; then
  command -v curl >/dev/null || die "读回基线需 curl(离线/首发请显式 --no-baseline)"
  command -v minisign >/dev/null || die "读回基线要验签,需 minisign(或 --no-baseline)"
  PUB_RB="${PUBKEY_FILE:-$HERE/updater-pubkey.pub}"
  [ -f "$PUB_RB" ] || die "读回基线要验签,缺公钥 $PUB_RB(或 --no-baseline)"
  RB_DIR="$(mktemp -d)"; CLEANUP_PATHS+=("$RB_DIR")
  rc=0; curl -fsSL --max-time 20 "$BASE_URL/$CHANNEL.json" -o "$RB_DIR/m.json" || rc=$?
  [ "$rc" = 0 ] || die "读回已发布 manifest 失败(rc=$rc,网络/端点/首发?)—— 拒绝(首发请显式 --no-baseline,不让瞬时网络错静默回退成计数倒退)"
  curl -fsSL --max-time 20 "$BASE_URL/$CHANNEL.json.minisig" -o "$RB_DIR/m.sig" \
    || die "已发布 manifest 有正文却拉不到 .minisig —— 拒绝采信未签名的基线"
  minisign -V -p "$PUB_RB" -m "$RB_DIR/m.json" -x "$RB_DIR/m.sig" >/dev/null 2>&1 \
    || die "已发布 manifest 验签失败 —— 拒绝把未经验证的输入洗进签名产物(疑投毒)"
  jq empty "$RB_DIR/m.json" 2>/dev/null || die "已发布 manifest 非合法 JSON"
  # 已验签 = 可信 → 作权威基线:counter/releases/floor 全从这里来。
  PREV_META="$(jq -r '(.metadata_version // 0) | floor' "$RB_DIR/m.json")"
  # (#196 R9 F2 加固)基线虽已验签,仍卡数字上界:防陈旧签名基线的越界 counter 让下方 -gt 比较崩(set -e)。
  [[ "$PREV_META" =~ ^[0-9]+$ ]] && [ "$PREV_META" -le "$META_MAX" ] \
    || die "读回基线 metadata_version=$PREV_META 非法(需 0..$META_MAX 整数)—— 拒绝"
  PREV_RELEASES="$(jq -c '.releases // []' "$RB_DIR/m.json")"
  BASE_FLOOR="$(jq -r '.rollback_floor // empty' "$RB_DIR/m.json")"
  BASE_REVOKED="$(jq -c '.revoked // []' "$RB_DIR/m.json")"
  BASE_HAS_REVOKED="$(jq -r 'has("revoked")' "$RB_DIR/m.json")"   # 缺字段 ≠ 空数组(迁移窗口判别)
  # (finding2) .channel 必须匹配 —— minisign 只签**字节**不签来源:一份合法签名的 beta.json 被
  # (误)放到 stable.json 的 URL,会被整份继承进 stable(beta 的低 floor / 预发布 releases 污染 stable)。
  BASE_CHANNEL="$(jq -r '.channel // empty' "$RB_DIR/m.json")"
  [ -z "$BASE_CHANNEL" ] || [ "$BASE_CHANNEL" = "$CHANNEL" ] \
    || die "读回基线 channel=$BASE_CHANNEL != 目标 $CHANNEL —— 拒绝(签名保真不保来源;疑 $BASE_CHANNEL 被喂到 $CHANNEL 的 URL)"
  # (finding1a) 拒绝**已过期**的基线 —— 验签只证真实性、不证新鲜度。重放一份旧的但签名合法的
  # manifest 能"撤销撤销"(把 rollback_floor/releases 退回旧值);过期检查把重放窗口从「永远」压到
  # EXPIRES_DAYS。缺 expires 也 fail-closed(无法验新鲜度)。跨 mac/linux 解析,镜像节点 :521-527。
  BASE_EXP="$(jq -r '.expires // empty' "$RB_DIR/m.json")"
  [ -n "$BASE_EXP" ] || die "读回基线缺 expires —— 无法验证新鲜度,拒绝(fail-closed)"
  # 先过严格 ISO-8601 正则(镜像节点 :504),再交给 date —— 否则 GNU date -d 会吃 `next year`
  # 这类相对表达式,给 readback 留一条解析歧义(Low)。
  [[ "$BASE_EXP" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] \
    || die "读回基线 expires 格式非法($BASE_EXP)—— 拒绝(需严格 ISO-8601 UTC)"
  base_exp_epoch="$(date -u -d "$BASE_EXP" +%s 2>/dev/null || date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$BASE_EXP" +%s 2>/dev/null || echo 0)"
  [ "$base_exp_epoch" -gt 0 ] || die "读回基线 expires 无法解析($BASE_EXP)—— 拒绝(fail-closed)"
  [ "$(date -u +%s)" -gt "$base_exp_epoch" ] && die "读回基线已过期($BASE_EXP)—— 疑重放旧签名 manifest 撤销撤销,拒绝(首发/离线用 --no-baseline)"
  # 存基线**原值**(门控/并集前)—— 日志与告警必须按**已验签基线**的真实值打,不能打并集后的
  # (#196 R5 Blocking2:旧日志打并集后 meta/releases 却标"已验签基线",正是复活没被发现的直接原因)。
  BASE_META="$PREV_META"; BASE_COUNT="$(printf '%s' "$PREV_RELEASES" | jq length)"
  echo "读回并**验签**已发布基线:metadata_version=$BASE_META releases=$BASE_COUNT rollback_floor=${BASE_FLOOR:-<none>} revoked=$(printf '%s' "$BASE_REVOKED" | jq length)" >&2
  # ── (#196 R9 B1) 未验证的本地 $OUT 绝不驱动不可逆字段 ───────────────────────────
  # 有已验签基线时,三个单调且全网永久的字段(metadata_version/rollback_floor/revoked)**只**来自
  # 已验签基线 + 显式 flag。未验证本地(= --trust-unsigned-local 的无签名 $OUT)若会**抬高**其中任一 →
  # **响亮 die 并点名该打哪个 flag**,绝不静默丢弃 —— 静默丢=镜像漏洞(被悄悄解除的撤销/被悄悄降低的
  # 围栏)。判据是**验证状态**不是来源:LOCAL_VERIFIED=1(签名+channel 双过)的本地照常参与 max/union
  # (它是脚本自己 :385 签的,counter 更高是诚实的;取基线反会在同一 counter 上产出两份合法签名 →
  # 节点 :534 只拒 `<` 不拒 `=`,两份都收)。此守卫只在 NO_BASELINE != 1 生效(--no-baseline 无基线可护,
  # flag 正是那时的合法播种途径)。
  if [ "$LOCAL_VERIFIED" != 1 ]; then
    [ "$LOCAL_META" -le "$PREV_META" ] \
      || die "未验证的本地 $OUT metadata_version=$LOCAL_META > 已验签基线 $PREV_META —— 拒绝(未签名本地不得抬高全网单调计数;要提 counter 请签名 $OUT,或 --no-baseline 首发)"
    [ "$(printf '%s' "$LOCAL_REVOKED" | jq 'length')" = 0 ] \
      || die "未验证的本地 $OUT 带 revoked=$(printf '%s' "$LOCAL_REVOKED" | jq -c .) —— 拒绝(撤销是全网永久操作,不接受未签名来源;请改用 --revoke <版本> 显式声明)"
    if [ -n "$LOCAL_FLOOR" ] && [ "$(semver_max "${BASE_FLOOR:-0.0.0}" "$LOCAL_FLOOR" | sed 's/^v//')" != "$(printf '%s' "${BASE_FLOOR:-0.0.0}" | sed 's/^v//')" ]; then
      die "未验证的本地 $OUT rollback_floor=$LOCAL_FLOOR 高于已验签基线 ${BASE_FLOOR:-<none>} —— 拒绝(围栏是全网永久单调操作;请改用 --rollback-floor $LOCAL_FLOOR 显式声明)"
    fi
  fi
  # counter 单调:max(本地,基线)。守卫已保证未验证本地到此处 LOCAL_META ≤ PREV_META(不会污染);
  # 已验证本地(T24 正控)的更高 counter 在此正当生效。releases/revoked 合并在 fi 之后统一做(#196 R6)。
  [ "$LOCAL_META" -gt "$PREV_META" ] && PREV_META="$LOCAL_META"
fi

# ── revoked:[] 墓碑:撤销/合并可判定的唯一机制(#196 R6/R7)────────────────────
# revoked = 单调 union(基线 + 本地 + --revoke),**永不缩小** —— 一份陈旧基线不能 un-revoke。
REVOKED="$(jq -cn --argjson base "$BASE_REVOKED" --argjson local "$LOCAL_REVOKED" --arg add "$REVOKE_LIST" '
  ($add | split(",") | map(select(length>0) | ltrimstr("v"))) as $a
  | (($base + $local | map(ltrimstr("v"))) + $a) | unique')"

# 本地独有 release 条目(基线无、本地有)——来自**未签名**的本地 $OUT,合并需披露 + 视迁移状态决定。
LOCAL_ONLY="$(jq -rn --argjson base "$PREV_RELEASES" --argjson local "$LOCAL_RELEASES" \
  '($base|map(.version)) as $bv | [$local[]|select((.version as $v|$bv|index($v))|not)|.version]|join(",")')"

# 迁移安全(#196 R7 finding1):已验签基线**缺 revoked 字段**(墓碑之前发布)时,老式"删条目"撤销
# 可能仍在生效且不可探测 → **不把本地独有条目并入**(它可能正是被老式撤销的),要求 --version/--revoke
# 重新声明。has("revoked")==false 与空数组区分开。
MERGE_LOCAL=1
if [ "$NO_BASELINE" != 1 ] && [ "$LOCAL_VERIFIED" != 1 ] && [ -n "$LOCAL_ONLY" ]; then
  # (#196 R9 B1 path4) 未验证本地**永远**不贡献 releases[](与 BASE_HAS_REVOKED 无关) —— releases 是
  # 可逆字段(丢了可 --version 重签),故 warn+drop 而非 die;不可逆的 counter/floor/revoked 已在上方守卫 die。
  MERGE_LOCAL=0
  echo "⚠️ 未验证的本地 $OUT 独有 release **不并入**产物:$LOCAL_ONLY(确为有效版本请签名 $OUT 或 --version 重签)" >&2
elif [ "$NO_BASELINE" != 1 ] && [ "$BASE_HAS_REVOKED" != true ] && [ -n "$LOCAL_ONLY" ]; then
  # 迁移:已验签基线**无 revoked 字段**(墓碑之前发布)→ 本地独有条目可能正是老式『删条目』撤销的,不并入。
  MERGE_LOCAL=0
  echo "⚠️ 迁移:已验签基线**无 revoked 字段**(墓碑之前发布)→ 本地独有条目**不并入**产物:$LOCAL_ONLY" >&2
  echo "   (可能是老式『删条目』撤销的;确为有效版本请 --version 重签,确为撤销请 --revoke 声明)" >&2
fi
# releases = 基线(+ 视 MERGE_LOCAL 并本地独有),再**滤掉 revoked**。堵死三条复活路 + 未签名条目不静默洗入。
if [ "$MERGE_LOCAL" = 1 ]; then
  [ -n "$LOCAL_ONLY" ] && echo "并入本地独有 \$OUT 条目:$LOCAL_ONLY($([ "$LOCAL_VERIFIED" = 1 ] && echo 已验签本地 || echo --no-baseline 播种),可判定)" >&2
  PREV_RELEASES="$(jq -cn --argjson base "$PREV_RELEASES" --argjson local "$LOCAL_RELEASES" --argjson rev "$REVOKED" '
    ($base | map(.version)) as $bv
    | ($base + ($local | map(select((.version as $v|$bv|index($v))|not))))
    | map(select(((.version|ltrimstr("v")) as $vv | $rev|index($vv))|not))')"
else
  PREV_RELEASES="$(jq -cn --argjson base "$PREV_RELEASES" --argjson rev "$REVOKED" \
    '$base | map(select(((.version|ltrimstr("v")) as $vv | $rev|index($vv))|not))')"
fi

# --no-baseline 安全(#196 R7 finding2):没读基线 → REVOKED 只来自本地 $OUT,若线上已撤销更多而本地
# $OUT 陈旧,这些撤销会丢失、并可能在更高 counter 上复活。响亮告警 + 打印将发出的 revoked。
if [ "$NO_BASELINE" = 1 ]; then
  echo "⚠️ --no-baseline:revoked 仅来自本地 \$OUT(未与线上比对)=$(printf '%s' "$REVOKED" | jq -c .) —— 若线上已撤销更多版本而本地 \$OUT 陈旧,那些撤销会丢失;确认 \$OUT 最新,或 --revoke 重述已发布的撤销集。" >&2
fi

# --version 撞 revoked → 拒(不能重签一个已撤销的版本)。纯撤销(无 VERSION)跳过本检查。
if [ -n "$VERSION" ] && printf '%s' "$REVOKED" | jq -e --arg v "${VERSION#v}" 'index($v) != null' >/dev/null 2>&1; then
  die "版本 $VERSION 在 revoked 墓碑里(已被撤销)—— 不能重新签发。要恢复需人工从 channel 的 revoked[] 移除(慎重)。"
fi
NEW_META=$((PREV_META + 1))
# (#196 R9 F2 加固)--no-baseline 下 PREV_META 全来自本地 $OUT,2^63-1 会溢出成负数并被真钥签出。
# LOCAL_META 读入已卡上界,这里再断言产物 counter 在合法区间(镜像节点 ratchet 语义,永不签负/越界)。
# NEW_META 必须仍 ≤ META_MAX(= 读回门),否则签出的东西下次自己读不回去(见 :156 不变量)。
# 到顶 = counter 空间耗尽,响亮 die(绝不静默饱和停在 MAX:那会让两份内容共享一个 counter)。
[ "$NEW_META" -gt 0 ] && [ "$NEW_META" -le "$META_MAX" ] \
  || die "metadata_version 已达上界(PREV_META=$PREV_META,NEW_META=$NEW_META > META_MAX=$META_MAX)—— counter 空间耗尽,拒绝签发(签出即读不回;不抬门、不静默饱和)"

# 缺省 min_version / requires_ta:从 prev releases 里**按 semver 取最高版本**那条继承
# (不是最近签发的 [0] —— 否则先签 0.30 再签热修 0.29.1 会让后续 0.31 静默继承 0.29.1 的
# 宽松 min_version,放宽升级闸门,让本该分步升级的节点跳级)。用 jq 数字化三段排序。
_highest_field() { # <field-name> <fallback>
  echo "$PREV_RELEASES" | jq -r --arg f "$1" --arg fb "$2" '
    def key: (.version|ltrimstr("v")|split(".")|map(tonumber? // 0));
    if length==0 then $fb
    else (sort_by(key) | last | .[$f]) as $v | ($v // $fb) end'
}
[ -n "$MIN_VERSION" ] || MIN_VERSION="$(_highest_field min_version 0.28.0)"
[ -n "$REQUIRES_TA" ] || REQUIRES_TA="$(_highest_field requires_ta_version 0.28.0)"
# rollback_floor:反回退**单调** —— 取 max(显式 --rollback-floor 或基线默认, 本地 floor, 基线 floor)。
# floor 是安全地板(围栏低于它的已知漏洞版本),只升不降;陈旧基线/本地都压不低它。撤销具体版本走
# revoked[] 墓碑、不动 floor。要真降 floor(un-fence)须人工:清本地 $OUT 且基线本身也低。
EXPLICIT_FLOOR="$ROLLBACK_FLOOR"
[ -n "$ROLLBACK_FLOOR" ] || ROLLBACK_FLOOR="${BASE_FLOOR:-0.28.0}"
ROLLBACK_FLOOR="$(semver_max "$ROLLBACK_FLOOR" "${LOCAL_FLOOR:-0.0.0}")"
ROLLBACK_FLOOR="$(semver_max "$ROLLBACK_FLOOR" "${BASE_FLOOR:-0.0.0}")"
# 显式 --rollback-floor 被反回退抬高 → 提示(不静默;#196 R6 Low)。sed 去 v 前缀免 v0.31.0 假告警。
if [ -n "$EXPLICIT_FLOOR" ] && [ "$(printf '%s' "$EXPLICIT_FLOOR" | sed 's/^v//')" != "$(printf '%s' "$ROLLBACK_FLOOR" | sed 's/^v//')" ]; then
  echo "⚠️ 显式 --rollback-floor $EXPLICIT_FLOOR 低于本地/基线 floor,已反回退抬到 $ROLLBACK_FLOOR(floor 只升不降)。" >&2
fi

# 跨 mac/linux 的 UTC 时间戳
NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if date -u -d "@0" >/dev/null 2>&1; then
  EXPIRES="$(date -u -d "+$EXPIRES_DAYS days" +%Y-%m-%dT%H:%M:%SZ)"      # GNU
else
  EXPIRES="$(date -u -v +"${EXPIRES_DAYS}"d +%Y-%m-%dT%H:%M:%SZ)"        # BSD/mac
fi

# ── 组装 release 条目 + 合并(同版本则替换,否则前插)+ 顶层字段 ─────────
if [ -n "$VERSION" ]; then
  NEW_RELEASE="$(jq -n \
    --arg version "$VERSION" --argjson security "$SECURITY" --arg severity "$SEVERITY" \
    --arg notes "$NOTES" --arg notes_url "$NOTES_URL" --argjson auto "$AUTO_APPLY" \
    --argjson ta_changed "$TA_CHANGED" --arg min "$MIN_VERSION" --arg reqta "$REQUIRES_TA" \
    --argjson proto "$PROTO_VERSION" --arg tarball "$TARBALL_URL" --arg sha "$SHA" '
    {version:$version, security:$security, severity:$severity, notes:$notes, notes_url:$notes_url,
     auto_apply_allowed:$auto, ta_changed:$ta_changed, min_version:$min,
     requires_ta_version:$reqta, proto_version:$proto, tarball:$tarball, sha256:$sha, canary_ring:[]}')"
else
  NEW_RELEASE=null   # 纯撤销:不发新版本(#196 R7 finding3),releases 就是基线滤 revoked 后的
fi

MANIFEST="$(jq -n \
  --argjson meta "$NEW_META" --arg gen "$NOW" --arg exp "$EXPIRES" --arg channel "$CHANNEL" \
  --arg floor "$ROLLBACK_FLOOR" --argjson prev "$PREV_RELEASES" --argjson new "$NEW_RELEASE" \
  --argjson revoked "$REVOKED" '
  {metadata_version:$meta, generated_at:$gen, expires:$exp, channel:$channel, rollback_floor:$floor,
   revoked:$revoked,
   releases: (if $new == null then $prev else [$new] + ($prev | map(select(.version != $new.version))) end)}')"

# ── schema 自检:谓词在**共享** schema.jq(#203),与节点 load_manifest 同一份 —— 漂移在语法层不可能
# (曾手写双份漂移 3 次,daemon #5 根因)。签发端更严:require_floor=true(floor 必是 string)+
# check_revoked_releases=true(releases 不得含 revoked;墓碑=唯一撤销机制)。test-release-sign.sh T1
# 仍会真跑节点 load_manifest 校验本产物,作为端到端防漂移网。缺文件 = fail-closed。
SCHEMA_JQ="${SCHEMA_JQ:-$HERE/schema.jq}"
[ -f "$SCHEMA_JQ" ] || { echo "缺共享 schema $SCHEMA_JQ —— 拒绝签发(应与 release-sign.sh 同目录)" >&2; exit 1; }
echo "$MANIFEST" | jq -e --argjson nmax "$NOTES_MAX" --argjson require_floor true --argjson check_revoked_releases true \
  --from-file "$SCHEMA_JQ" >/dev/null \
  || { echo "组装出的 manifest 未过 schema 自检" >&2; echo "$MANIFEST" | jq . >&2; exit 1; }
[ "$(printf '%s' "$MANIFEST" | jq '.releases|length')" -eq 0 ] && echo "⚠️ 本次签发后 channel **无任何 release**(全部被撤销)—— 节点将无更新可装;确认这是预期(如全版本投毒的紧急撤销)。" >&2

# 进度/摘要一律 → stderr,让 dry-run 的 **stdout 是干净的 manifest JSON**(可直接 `| jq`)。
{
  echo "   version=$VERSION  metadata_version=${PREV_META}->${NEW_META}  sha256=$SHA"
  echo "   severity=$SEVERITY security=$SECURITY ta_changed=$TA_CHANGED expires=$EXPIRES"
  echo "   min_version=$MIN_VERSION requires_ta=$REQUIRES_TA rollback_floor=$ROLLBACK_FLOOR"
  [ "$TA_CHANGED" = "true" ] && echo "   ⚠️ ta_changed=true:节点将拒绝在线 apply,只能 OOB 刷 TA。"
} >&2

if [ "$DRY_RUN" = 1 ]; then
  # ⚠️ 变量后紧跟多字节字符(如 、)必须 ${VAR} 括起 —— macOS 自带 bash 3.2 的变量名扫描器
  # 非多字节感知,`$OUT、` 会把 、 吞进变量名,set -u 下直接 abort(签名机正是 Mac,dry-run 全废)。
  echo "── dry-run:以下为组装结果(**未写 ${OUT}、未签名**)──" >&2
  echo "$MANIFEST" | jq .   # stdout:纯 manifest JSON
  echo "（dry-run 结束:不产生任何副作用。去掉 --dry-run 才会签名 + 原子写 ${OUT}/.minisig)" >&2
  exit 0
fi

# ── 写临时文件 → 签名 → 自验 → 原子替换(签名成功前绝不动 $OUT/.minisig)───────
mkdir -p "$(dirname "$OUT")"
TMP_JSON="$(mktemp "${OUT}.tmp.XXXXXX")"
TMP_SIG="$TMP_JSON.minisig"
CLEANUP_PATHS+=("$TMP_JSON" "$TMP_SIG")   # 走统一 cleanup trap(失败也清,且不丢 RB_DIR 清理)
echo "$MANIFEST" | jq . > "$TMP_JSON"

echo "→ 用 $SECKEY 签名(将提示输入私钥密码)…" >&2
minisign -S -s "$SECKEY" -m "$TMP_JSON" -x "$TMP_SIG" -c "aastar node channel manifest ($CHANNEL v$VERSION)"
# 自验(仓库公钥;缺公钥在前置已 die,这里必存在)——不匹配即私钥用错,别落盘。
minisign -V -p "$PUB" -m "$TMP_JSON" -x "$TMP_SIG" >/dev/null \
  || die "自验失败!签名与仓库公钥不匹配 —— 私钥是否用错?(未触碰 ${OUT})"

# 原子替换:先落 .minisig 再落正文 —— 两个 mv 无法**联合**原子,但这个顺序下,任一时刻能被读到的
# 组合都不会是「新正文 + 旧签名」(节点先取 json 再验 sig,失败即 fail-closed 重试,不砖)。
# 上传步骤同理:先传 .minisig 再传 .json。
mv -f "$TMP_SIG"  "$OUT.minisig"
mv -f "$TMP_JSON" "$OUT"
echo "✅ 已签发并原子写入:"
echo "   $OUT"
echo "   $OUT.minisig(仓库公钥自验通过)"

cat <<EOF

下一步(发布):
  1. 把 tarball 传到 release:
       gh release create airaccount-node-v$VERSION "$TARBALL" -t "airaccount-node v$VERSION" -n "$NOTES"
     (或 gh release upload airaccount-node-v$VERSION "$TARBALL")
  2. 把签名 manifest 传到节点会拉的 URL(AU_MANIFEST_BASE = $BASE_URL)。两文件非单次原子切换 →
     **任何顺序都有一个有界 fail-closed 窗口**(先传 .minisig=「新签名+旧正文」,先传 .json=「新正文+
     旧签名」,节点两种都验签失败、拒绝重试,不砖)。彻底消除需单次原子切换(发到带版本号的路径 +
     原子改指针)。当前建议先传 .minisig(与本地原子写入同序,窗口方向一致):
       $OUT.minisig   ← 先
       $OUT           ← 后
  ⚠️ metadata_version / rollback_floor / releases 是防回滚的**权威来源**:反回退锚点靠本地 $OUT,
     而它未入库时新 clone/换签名机上不存在(见 R4 Blocking2)——强烈建议把 $OUT + .minisig **入库**,
     并让 --base-url 默认指向该仓库路径;仅靠 readback 的那一路在新机器上无本地锚点。
EOF
