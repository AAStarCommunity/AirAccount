#!/usr/bin/env bash
# build-register-bundle.sh (#21 / 闭合 #19 板侧打包缺口)
#
# 把 register-node.mjs + @aastar/operator + @aastar/core + viem 用 esbuild bundle 成
# 单文件 register-node.bundle.mjs(~2MB,tree-shaken,自包含)。板侧只需 `node register-node.bundle.mjs`,
# 无 node_modules / 无 SDK 解析问题(纯 JS,无 native 依赖→跨 arch 可用)。
#
# 用:  ./build-register-bundle.sh [输出路径]        # 默认 ./register-node.bundle.mjs
#      AASTAR_SDK_DIR=/path/to/aastar-sdk ./build-register-bundle.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
SDK="${AASTAR_SDK_DIR:-$(cd "$HERE/../../.." && pwd)/aastar-sdk}"   # 默认同级 aastar-sdk
OUT="${1:-$HERE/register-node.bundle.mjs}"
# esbuild 在 $STAGE 里跑,相对输出路径会写进临时目录被 trap 删(却仍打 ✅) → 转绝对路径。
case "$OUT" in /*) ;; *) OUT="$PWD/$OUT" ;; esac

[ -d "$SDK/packages/operator" ] || { echo "找不到 aastar-sdk: $SDK(设 AASTAR_SDK_DIR)"; exit 1; }

# 确保 operator+core 已构建(dist 在)。
if [ ! -f "$SDK/packages/operator/dist/index.js" ] || [ ! -f "$SDK/packages/core/dist/index.js" ]; then
  echo "构建 @aastar/operator + core …"
  ( cd "$SDK" && pnpm --filter @aastar/operator --filter @aastar/core build )
fi

ESB="$SDK/node_modules/.bin/esbuild"
[ -x "$ESB" ] || ESB="$(find "$SDK/node_modules/.pnpm" -type f -path '*/esbuild/bin/esbuild' 2>/dev/null | head -1)"
[ -x "$ESB" ] || { echo "找不到 esbuild(在 aastar-sdk 装 devDeps)"; exit 1; }

SDK_VER="$(node -p "require('$SDK/packages/operator/package.json').version" 2>/dev/null || echo unknown)"

# 临时 staging:符号链接让 esbuild 解析到 SDK 的 workspace 包 + viem。
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/node_modules/@aastar"
ln -s "$SDK/node_modules/viem"      "$STAGE/node_modules/viem"
ln -s "$SDK/packages/operator"      "$STAGE/node_modules/@aastar/operator"
ln -s "$SDK/packages/core"          "$STAGE/node_modules/@aastar/core"
cp "$HERE/register-node.mjs"        "$STAGE/register-node.mjs"

( cd "$STAGE" && "$ESB" register-node.mjs \
    --bundle --platform=node --format=esm --outfile="$OUT" \
    --banner:js="// register-node.bundle.mjs — 自动生成(build-register-bundle.sh),勿手改。@aastar/operator@${SDK_VER} + viem via esbuild。" )

echo "✅ 产出 $OUT ($(du -h "$OUT" | cut -f1) · @aastar/operator@${SDK_VER})"
echo "   板侧用法: node register-node.bundle.mjs --dry-run   (无需 node_modules)"
