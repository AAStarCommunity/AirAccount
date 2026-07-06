#!/usr/bin/env bash
# deploy-dvt.sh — 把 DVT(aNode)从源码 build + 部署到 imx93 板,与 KMS 隔离共存。
#
# 为什么 bare-node 不用 Docker:嵌入式板的 Docker 常残缺(实测 imx93:legacy
# builder 在 ext4 上 xattr 导出层失败 / 无 buildx / 内核无 iptables raw 表)。
# bare-node 更省内存、更 robust,是受限板的推荐姿势。
#
# 用法:
#   ./deploy-dvt.sh --config community.toml --dvt-repo ~/Dev/aastar/YetAnotherAA-Validator
#
# 前置:操作机能 SSH 到板子(Tailscale);DVT repo 干净(git archive 该 tag)。
# KMS 不由本脚本部署(走 mx93-build/deploy.sh);本脚本只校验 KMS 在位。
set -euo pipefail

CONFIG=""; DVT_REPO=""
while [ $# -gt 0 ]; do case "$1" in
  --config) CONFIG="$2"; shift 2;;
  --dvt-repo) DVT_REPO="$2"; shift 2;;
  *) echo "unknown arg: $1"; exit 2;;
esac; done
[ -f "$CONFIG" ] || { echo "need --config <community.toml>"; exit 2; }
[ -d "$DVT_REPO/.git" ] || { echo "need --dvt-repo <path to YetAnotherAA-Validator>"; exit 2; }

# 段感知 TOML 取值:cfg <section> <key>  —— awk 跨 BSD(macOS)/GNU 一致,
# 消除歧义([kms].version vs [dvt].version),健壮剥引号/空白/行内注释。
cfg() {
  awk -v s="$1" -v k="$2" '
    /^[[:space:]]*\[/ { insec = ($0 ~ "^[[:space:]]*\\["s"\\]") }
    insec && $0 ~ ("^[[:space:]]*"k"[[:space:]]*=") {
      sub(/^[^=]*=[[:space:]]*/, ""); sub(/[[:space:]]*#.*$/, "");
      gsub(/^["[:space:]]+|["[:space:]]+$/, ""); print; exit
    }' "$CONFIG"
}

BOARD=$(cfg community board_ssh);   KEY=$(cfg community ssh_key)
DVT_VER=$(cfg dvt version);         PORT=$(cfg dvt port)
VALIDATOR=$(cfg dvt validator_contract_address)
POLICY=$(cfg dvt policy_enabled);   NODE_NAME=$(cfg dvt node_name)
RPC=$(cfg chain eth_rpc_url);       ENTRY=$(cfg chain entry_point)
NODE_VER=$(cfg runtime node_version); NODE_DIR=$(cfg runtime node_dir); DVT_DIR=$(cfg runtime dvt_dir)
KEY="${KEY/#\~/$HOME}"
[ -n "$BOARD" ] && [ -n "$DVT_VER" ] && [ -n "$PORT" ] && [ -n "$DVT_DIR" ] || { echo "config parse failed (BOARD=$BOARD DVT_VER=$DVT_VER PORT=$PORT)"; exit 2; }
SSH="ssh -o ConnectTimeout=15 -i $KEY $BOARD"
SCP="scp -o ConnectTimeout=15 -i $KEY"

echo "▶ DVT v$DVT_VER → $BOARD:$PORT (KMS 共存, bare-node)"

# 1. 板上确保 glibc node
$SSH "test -x $NODE_DIR/bin/node" || {
  echo "▶ 安装 Node $NODE_VER (glibc arm64)"
  $SSH "curl -fsSL -o /tmp/node.tar.xz https://nodejs.org/dist/$NODE_VER/node-$NODE_VER-linux-arm64.tar.xz && rm -rf $NODE_DIR && mkdir -p $NODE_DIR && tar xJf /tmp/node.tar.xz -C $NODE_DIR --strip-components=1"
}
$SSH "$NODE_DIR/bin/node --version"

# 2. 导出 DVT vX.Y.Z 源码 → 上板
#    ⚠️ 不 rm -rf 整个目录:node_state.json(节点 BLS 身份)+ dvt.env 必须跨重部署保留。
#    git archive 的 tar 不含这两个文件,解压覆盖源码即可,它们原地存活。
echo "▶ git archive v$DVT_VER → 上板(保留 node_state.json / dvt.env)"
( cd "$DVT_REPO" && git archive --format=tar.gz -o /tmp/dvt-src.tar.gz "v$DVT_VER" )
$SSH "mkdir -p $DVT_DIR"
$SCP /tmp/dvt-src.tar.gz "$BOARD:$DVT_DIR/src.tar.gz"
# ⚠️ --exclude node_state.json:DVT 仓库 committed 了一个 bls-node-001 测试 fixture 密钥
#    (公共 BLS_TEST key,"谁都能签")。绝不能让它覆盖本节点的独立密钥。排除后:
#    真实独立 node_state 存活;首次部署则由下方 Step 4 生成。
$SSH "cd $DVT_DIR && tar xzf src.tar.gz --exclude=node_state.json --exclude=./node_state.json && rm -f node_state.json.repo && grep '\"version\"' package.json | head -1"

# 3. build(板上 bare node)
echo "▶ npm ci && npm run build (A55 上较慢)"
$SSH "export PATH=$NODE_DIR/bin:\$PATH; cd $DVT_DIR && npm ci && npm run build && test -f dist/main.js && echo build-ok"

# 4. 生成独立 BLS12-381 node 密钥(已存在则跳过,不复用不覆盖)
echo "▶ node_state.json (独立 BLS 密钥)"
$SSH "test -f $DVT_DIR/node_state.json" && echo "  已存在,跳过" || \
$SSH "export PATH=$NODE_DIR/bin:\$PATH; cd $DVT_DIR && node --input-type=module -e '
import { bls12_381 as bls } from \"@noble/curves/bls12-381.js\";
import { randomBytes } from \"crypto\"; import { writeFileSync } from \"fs\";
const s=bls.longSignatures; let sk; do{sk=randomBytes(32);try{s.getPublicKey(sk);break}catch{}}while(true);
writeFileSync(\"node_state.json\", JSON.stringify({nodeId:\"0x\"+randomBytes(32).toString(\"hex\"),nodeName:\"$NODE_NAME\",privateKey:\"0x\"+Buffer.from(sk).toString(\"hex\"),publicKey:s.getPublicKey(sk).toHex(),createdAt:new Date().toISOString(),description:\"production DVT node\"},null,2));
console.log(\"BLS pubkey\", s.getPublicKey(sk).toHex().slice(0,24)+\"...\");' && chmod 600 node_state.json"

# 5. 写 env
echo "▶ dvt.env"
$SSH "cat > $DVT_DIR/dvt.env <<EOF
NODE_ENV=production
PORT=$PORT
ETH_RPC_URL=$RPC
VALIDATOR_CONTRACT_ADDRESS=$VALIDATOR
ENTRY_POINT_ADDRESS=$ENTRY
NODE_STATE_FILE=$DVT_DIR/node_state.json
POLICY_ENABLED=$POLICY
EOF"

# 6. systemd(隔离,不碰 kms-api.service)
echo "▶ dvt.service"
$SSH "cat > /etc/systemd/system/dvt.service <<EOF
[Unit]
Description=AAStar DVT (aNode) v$DVT_VER — bare-node, co-located with KMS (isolated)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$DVT_DIR
EnvironmentFile=$DVT_DIR/dvt.env
ExecStart=$NODE_DIR/bin/node $DVT_DIR/dist/main.js
Restart=on-failure
RestartSec=5
StandardOutput=append:$DVT_DIR/dvt.log
StandardError=append:$DVT_DIR/dvt.log

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload && systemctl enable --now dvt.service && systemctl restart dvt.service"

# 7. 验收
echo "▶ 等 25s boot + health"
$SSH "sleep 25; echo -n 'DVT  '; curl -s -m5 http://127.0.0.1:$PORT/health | head -c 80; echo; echo -n 'node '; curl -s -m5 http://127.0.0.1:$PORT/node/info | head -c 120; echo; echo -n 'KMS  '; curl -s -m5 http://127.0.0.1:3000/health | head -c 40; echo; free -m | awk '/Mem/{print \"available: \"\$7\"MB\"}'"
echo "✅ DVT v$DVT_VER 部署完成,与 KMS 共存。BLS 验证见 kms/docs/kms-dvt-imx93-deploy.md §E"
