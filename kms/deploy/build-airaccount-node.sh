#!/usr/bin/env bash
# build-airaccount-node.sh (#21) — 可复现地组装 airaccount-node release bundle。
#
# 把【当前仓库的】node-setup(含修好的 register-node.mjs + esbuild bundle + finalize-helper +
# sudoers)、deploy(4-profile installer + community-profiles)、KMS TA/CA(从 airaccount-kms
# 产物取,不重编)组装成 airaccount-node-<ver>.tar.gz。社区 installer 下它 → 板上就带全部修复。
#
# 用:
#   KMS_SRC=<airaccount-kms 解包目录或 tarball> VERSION=v0.29.0 ./build-airaccount-node.sh [输出目录]
#   AASTAR_SDK_DIR=/path/to/aastar-sdk   # register bundle 用(默认同级 aastar-sdk)
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"          # kms/deploy
REPO="$(cd "$HERE/../.." && pwd)"
NODE_SETUP="$REPO/kms/node-setup"
VERSION="${VERSION:-v0.0.0-dev}"
OUTDIR="${1:-$REPO/dist}"
KMS_SRC="${KMS_SRC:-}"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

log() { echo -e "\033[0;34m[build-node]\033[0m $*"; }
die() { echo "build-node: $*" >&2; exit 1; }

STAGE="$(mktemp -d)"; trap 'rm -rf "$STAGE"' EXIT
B="$STAGE/airaccount-node-$VERSION"
mkdir -p "$B"/{kms,node-setup,deploy}

# ── 1. KMS TA/CA(从 airaccount-kms 产物取,不重编)────────────────────
[ -n "$KMS_SRC" ] || die "需 KMS_SRC=<airaccount-kms 目录或 .tar.gz>(提供 TA/CA)"
KDIR="$KMS_SRC"
if [[ "$KMS_SRC" == *.tar.gz ]]; then
  tar xzf "$KMS_SRC" -C "$STAGE/kmssrc" 2>/dev/null || { mkdir -p "$STAGE/kmssrc"; tar xzf "$KMS_SRC" -C "$STAGE/kmssrc"; }
  KDIR="$(find "$STAGE/kmssrc" -maxdepth 2 -name "$UUID.ta" -exec dirname {} \; | head -1)"
fi
[ -f "$KDIR/$UUID.ta" ] || die "KMS_SRC 里找不到 $UUID.ta"
log "KMS TA/CA ← $KDIR"
cp "$KDIR/$UUID.ta" "$B/kms/"
cp "$KDIR/kms-api-server" "$B/kms/" 2>/dev/null || die "缺 kms-api-server"
for f in kms-api.service attestation-measurements.json; do
  [ -f "$KDIR/$f" ] && cp "$KDIR/$f" "$B/kms/" || true
done

# ── 2. node-setup(当前仓库全量 + esbuild register bundle)──────────────
log "node-setup(含修复)+ 构建 register bundle"
for f in setup-server.py setup.html register-node.mjs aastar-node-setup.service \
         finalize-helper.sh aastar-node-setup.sudoers \
         aastar-kms-selfinit.sh aastar-kms-selfinit.service; do
  [ -f "$NODE_SETUP/$f" ] && cp "$NODE_SETUP/$f" "$B/node-setup/" || true
done
# esbuild 单文件 register bundle(板上无 node_modules 也能跑;含 camelCase 修复)
"$NODE_SETUP/build-register-bundle.sh" "$B/node-setup/register-node.bundle.mjs" >/dev/null \
  || die "register bundle 构建失败(检查 aastar-sdk / esbuild)"

# ── 3. deploy(4-profile installer + community-profiles)────────────────
log "deploy installer + profiles"
cp "$HERE/aastar-node-installer.sh" "$B/deploy/"
[ -d "$HERE/community-profiles" ] && cp -r "$HERE/community-profiles" "$B/deploy/"

# ── 4. BUNDLE.md + 打包 ──────────────────────────────────────────────
cat > "$B/BUNDLE.md" <<EOF
# airaccount-node $VERSION
KMS(TA/CA) + node-setup(向导 + register-node esbuild bundle,含 #19 camelCase 修复 + #23 finalize-helper)
+ 4-profile installer + community-profiles。社区: \`aastar-node-installer.sh\` 下本 bundle 装。
构建: build-airaccount-node.sh @ $(cd "$REPO" && git rev-parse --short HEAD 2>/dev/null || echo '?')
EOF
mkdir -p "$OUTDIR"
TAR="$OUTDIR/airaccount-node-$VERSION.tar.gz"
tar czf "$TAR" -C "$STAGE" "airaccount-node-$VERSION"
log "✅ 产出 $TAR ($(du -h "$TAR" | cut -f1))"
echo "  含: $(tar tzf "$TAR" | grep -cE 'node-setup/register-node.bundle.mjs|node-setup/register-node.mjs') 个 register 组件"
