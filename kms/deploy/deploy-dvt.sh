#!/usr/bin/env bash
# deploy-dvt.sh — 把 DVT(aNode)从源码 build + 安全部署到 imx93 板,与 KMS 隔离共存。
#
# 安全模型(生产):
#   - BLS 私钥 EIP-2335 加密 keystore(盘上只密文,scrypt/pbkdf2 KDF)。
#   - NODE_KEY_PASSPHRASE 部署时手动输入 → 只进 tmpfs(/run/dvt/pass,内存),绝不落盘。
#   - 崩溃:systemd 从 tmpfs 自动重启;断电重启:tmpfs 清空 → 须 dvt-unlock.sh 重输密码。
#   - 硬盘离线也拿不到 BLS 私钥(密文 + 无密码)。
#
# 为什么 bare-node 不用 Docker:嵌入式板 Docker 常残缺(imx93 实测:legacy builder
#   ext4 xattr 导出层失败 / 无 buildx / 内核无 iptables raw 表)。
#
# 用法:
#   ./deploy-dvt.sh --config community.toml --dvt-repo ~/Dev/aastar/YetAnotherAA-Validator
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

# 段感知 TOML 取值:cfg <section> <key>(awk 跨 BSD/GNU 一致,剥引号/空白/注释)
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
POLICY=$(cfg dvt policy_enabled);   NODE_NAME=$(cfg dvt node_name); KDF=$(cfg dvt kdf)
RPC=$(cfg chain eth_rpc_url);       ENTRY=$(cfg chain entry_point)
ACTIVE=$(cfg contracts active)
VALIDATOR=$(cfg "contracts_${ACTIVE}" validator)
NODE_VER=$(cfg runtime node_version); NODE_DIR=$(cfg runtime node_dir); DVT_DIR=$(cfg runtime dvt_dir)
KEY="${KEY/#\~/$HOME}"; KDF="${KDF:-scrypt}"
# config 校验(缺关键项就拒)
for v in BOARD DVT_VER PORT VALIDATOR RPC DVT_DIR; do
  eval "val=\$$v"; [ -n "$val" ] || { echo "config parse failed: $v 空(检查 [contracts_$ACTIVE].validator 等)"; exit 2; }
done
SSH="ssh -o ConnectTimeout=15 -i $KEY $BOARD"
SCP="scp -o ConnectTimeout=15 -i $KEY"

echo "▶ DVT v$DVT_VER → $BOARD:$PORT (KMS 共存, bare-node, 加密 keystore, contracts=$ACTIVE)"

# 1. glibc node
$SSH "test -x $NODE_DIR/bin/node" || {
  echo "▶ 安装 Node $NODE_VER (glibc arm64)"
  $SSH "curl -fsSL -o /tmp/node.tar.xz https://nodejs.org/dist/$NODE_VER/node-$NODE_VER-linux-arm64.tar.xz && rm -rf $NODE_DIR && mkdir -p $NODE_DIR && tar xJf /tmp/node.tar.xz -C $NODE_DIR --strip-components=1"
}
$SSH "$NODE_DIR/bin/node --version"

# 2. 源码上板(--exclude node_state.json:保留真实密钥,避开仓库 committed 的测试 fixture)
echo "▶ git archive v$DVT_VER → 上板(保留 node_state.json / dvt.env)"
( cd "$DVT_REPO" && git archive --format=tar.gz -o /tmp/dvt-src.tar.gz "v$DVT_VER" )
$SSH "mkdir -p $DVT_DIR"
$SCP /tmp/dvt-src.tar.gz "$BOARD:$DVT_DIR/src.tar.gz"
$SSH "cd $DVT_DIR && tar xzf src.tar.gz --exclude=node_state.json --exclude=./node_state.json && grep '\"version\"' package.json | head -1"

# 3. build
echo "▶ npm ci && npm run build (A55 上较慢)"
$SSH "export PATH=$NODE_DIR/bin:\$PATH; cd $DVT_DIR && npm ci && npm run build && test -f dist/main.js && echo build-ok"

# 4. 手动输入 keystore 密码 → 只进 tmpfs(绝不落盘、不上 argv)
echo "▶ BLS keystore 密码(手动,不落盘)"
printf "  NODE_KEY_PASSPHRASE: " >&2; read -rs PASS; echo >&2
[ -n "$PASS" ] || { echo "空密码"; exit 2; }
$SSH "mkdir -p /run/dvt && chmod 700 /run/dvt"
printf 'NODE_KEY_PASSPHRASE=%s\n' "$PASS" | $SSH "cat > /run/dvt/pass && chmod 600 /run/dvt/pass"

# 5. 生成独立 BLS 密钥(缺则生成)→ 加密成 EIP-2335 keystore(明文即刻抹除)
echo "▶ node_state.json(独立 BLS 密钥 + EIP-2335 加密, kdf=$KDF)"
$SSH "export PATH=$NODE_DIR/bin:\$PATH; cd $DVT_DIR
  if [ ! -f node_state.json ]; then
    node --input-type=module -e '
import { bls12_381 as bls } from \"@noble/curves/bls12-381.js\";
import { randomBytes } from \"crypto\"; import { writeFileSync } from \"fs\";
const s=bls.longSignatures; let sk; do{sk=randomBytes(32);try{s.getPublicKey(sk);break}catch{}}while(true);
writeFileSync(\"node_state.json\", JSON.stringify({nodeId:\"0x\"+randomBytes(32).toString(\"hex\"),nodeName:\"$NODE_NAME\",privateKey:\"0x\"+Buffer.from(sk).toString(\"hex\"),publicKey:s.getPublicKey(sk).toHex(),createdAt:new Date().toISOString(),description:\"production DVT node\"},null,2));
console.log(\"  new BLS pubkey\", s.getPublicKey(sk).toHex().slice(0,24)+\"...\");'
  fi
  # 若还是明文(有 privateKey 无 keystore)→ 加密 + 抹明文
  if node -e 'const s=require(\"./node_state.json\"); process.exit(s.keystore?0:1)'; then
    echo '  已是加密 keystore,跳过'
  else
    set -a; . /run/dvt/pass; set +a
    KDF=$KDF node scripts/encrypt-node-key.mjs node_state.json >/dev/null
    shred -u node_state.json.bak 2>/dev/null || rm -f node_state.json.bak
    echo '  已加密为 EIP-2335 keystore,明文已抹'
  fi
  chmod 600 node_state.json"

# 6. env(非秘密)
echo "▶ dvt.env"
$SSH "cat > $DVT_DIR/dvt.env <<EOF
NODE_ENV=production
PORT=$PORT
ETH_RPC_URL=$RPC
VALIDATOR_CONTRACT_ADDRESS=$VALIDATOR
ENTRY_POINT_ADDRESS=$ENTRY
NODE_STATE_FILE=$DVT_DIR/node_state.json
POLICY_ENABLED=${POLICY:-false}
EOF
chmod 600 $DVT_DIR/dvt.env"

# 7. 加固 systemd(禁开机自启:断电须人工 unlock;崩溃从 tmpfs 自动重启)
echo "▶ dvt.service(加固 + 加密 keystore)"
$SSH "cat > /etc/systemd/system/dvt.service <<EOF
[Unit]
Description=AAStar DVT (aNode) v$DVT_VER — bare-node, encrypted BLS keystore (isolated)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$DVT_DIR
EnvironmentFile=$DVT_DIR/dvt.env
EnvironmentFile=/run/dvt/pass
ExecStart=$NODE_DIR/bin/node $DVT_DIR/dist/main.js
Restart=on-failure
RestartSec=5
StandardOutput=append:$DVT_DIR/dvt.log
StandardError=append:$DVT_DIR/dvt.log
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
ReadWritePaths=$DVT_DIR
[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl disable dvt.service 2>/dev/null || true   # 不开机自启(断电须人工输密码)
systemctl reset-failed dvt.service 2>/dev/null || true
systemctl restart dvt.service"
unset PASS

# 8. 验收
echo "▶ 等 28s boot + health"
$SSH "sleep 28; echo -n 'DVT  '; curl -s -m5 http://127.0.0.1:$PORT/health | head -c 70; echo; echo -n 'node '; curl -s -m5 http://127.0.0.1:$PORT/node/info | grep -oE '\"publicKey\":\"[^\"]{0,22}'; echo; echo -n 'KMS  '; curl -s -m5 http://127.0.0.1:3000/health | grep -oE '\"status\":\"[^\"]*\"'; grep -aiE 'encrypted keystore' $DVT_DIR/dvt.log | tail -1 | sed 's/\x1b\[[0-9;]*m//g'"

echo "✅ DVT v$DVT_VER 安全部署完成(加密 keystore + tmpfs 密码 + 加固)。"
echo "   断电重启后运维 unlock:ssh $BOARD 'bash /opt/dvt-build/dvt-unlock.sh'(见 kms/deploy/dvt-unlock.sh)"
echo "   链上注册(需 operator=validator owner):配 ETH_PRIVATE_KEY 后 curl -XPOST 127.0.0.1:$PORT/node/register"
