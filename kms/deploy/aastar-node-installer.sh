#!/usr/bin/env bash
# aastar-node-installer — 在刷好基础 OP-TEE 镜像的 FRDM-IMX93 上一条命令装 KMS+DVT 节点。
# 社区上手 Phase 2 路径 A(见 kms/docs/phase2-image.md)。
#
#   curl -fsSL .../aastar-node-installer.sh | sudo bash
#   curl -fsSL .../aastar-node-installer.sh | sudo VERSION=v0.28.1 bash
#
# 幂等:可重复跑(升级)。装完:KMS+DVT 起来、首启向导 aastar-node-setup 在 :8088 等配置。
# 前提:基础镜像已含 OP-TEE + tee-supplicant;有网;root。
set -euo pipefail

REPO="AAStarCommunity/AirAccount"
VERSION="${VERSION:-latest}"                       # airaccount-node release tag,或 latest
DVT_REPO="AAStarCommunity/YetAnotherAA-Validator"
DVT_TAG="${DVT_TAG:-v1.10.0}"                       # bundle 钉的 DVT release
TA_DIR="/lib/optee_armtz"
CA_DIR="/root/AirAccount/target/release"
NODE_SETUP_DIR="/opt/aastar/node-setup"
DVT_DIR="/opt/aastar/dvt"
CONFIG_DIR="/etc/airaccount"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

log() { echo -e "\033[0;34m[installer]\033[0m $*"; }
die() { echo -e "\033[0;31m[installer] $*\033[0m" >&2; exit 1; }
[[ $EUID -eq 0 ]] || die "需 root(sudo)"
command -v tee-supplicant >/dev/null 2>&1 || ls /usr/sbin/tee-supplicant >/dev/null 2>&1 || \
  die "没检测到 OP-TEE(tee-supplicant)—— 请先刷 NXP FRDM-IMX93 OP-TEE 基础镜像"
for c in curl tar; do command -v $c >/dev/null || die "缺 $c"; done

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

# ── 1. 下 airaccount-node release ────────────────────────────────────
if [[ "$VERSION" == "latest" ]]; then
  URL="https://github.com/$REPO/releases/latest/download/airaccount-node-v0.28.1.tar.gz"
else
  URL="https://github.com/$REPO/releases/download/airaccount-node-$VERSION/airaccount-node-$VERSION.tar.gz"
fi
log "下载 $URL"
curl -fSL "$URL" -o "$WORK/node.tgz" || die "下载失败(检查 VERSION / 网络)"
tar -xzf "$WORK/node.tgz" -C "$WORK"
BUNDLE="$(find "$WORK" -maxdepth 1 -type d -name 'airaccount-node-*')"
[[ -d "$BUNDLE/kms" ]] || die "release 结构异常(缺 kms/)"

# ── 2. 装 KMS(TA + CA + attestation manifest)────────────────────────
log "装 KMS TA/CA"
install -m 0444 "$BUNDLE/kms/$UUID.ta" "$TA_DIR/$UUID.ta"
mkdir -p "$CA_DIR"; install -m 0755 "$BUNDLE/kms/kms-api-server" "$CA_DIR/kms-api-server"
mkdir -p "$CONFIG_DIR"
[[ -f "$BUNDLE/kms/attestation-measurements.json" ]] && \
  install -m 0444 "$BUNDLE/kms/attestation-measurements.json" "$CONFIG_DIR/attestation-measurements.json" || true

# ── 3. 装 node-setup 向导 ────────────────────────────────────────────
log "装 aastar-node-setup 向导"
mkdir -p "$NODE_SETUP_DIR"
# 向导在仓库 kms/node-setup;release 里若没带则从仓库拉。
if [[ -d "$BUNDLE/node-setup" ]]; then cp -r "$BUNDLE/node-setup/." "$NODE_SETUP_DIR/"; else
  curl -fsSL "https://raw.githubusercontent.com/$REPO/main/kms/node-setup/setup-server.py" -o "$NODE_SETUP_DIR/setup-server.py"
  curl -fsSL "https://raw.githubusercontent.com/$REPO/main/kms/node-setup/setup.html" -o "$NODE_SETUP_DIR/setup.html"
fi

# ── 4. 装 DVT(clone + build v1.10.0)─────────────────────────────────
# DVT 是源码 release(NestJS + Rust signer)→ 在板上 build。重,但一次性。
if [[ ! -d "$DVT_DIR/.git" ]]; then
  log "clone DVT $DVT_TAG(NestJS + Rust signer,build 较慢)"
  git clone --depth 1 --branch "$DVT_TAG" "https://github.com/$DVT_REPO.git" "$DVT_DIR" \
    || die "DVT clone 失败"
fi
log "DVT 部署(交给 deploy-dvt.sh 处理 build/keystore/systemd)"
if [[ -x "$DVT_DIR/deploy/deploy-dvt.sh" ]]; then
  ( cd "$DVT_DIR" && ./deploy/deploy-dvt.sh ) || log "⚠️ deploy-dvt.sh 返回非零,检查 DVT 日志"
else
  log "⚠️ DVT deploy 脚本缺失,DVT 需手动 build(npm ci && npm run build + cargo build signer)"
fi

# ── 5. systemd units + 首启状态 ──────────────────────────────────────
log "装 systemd units"
[[ -f "$BUNDLE/kms/kms-api.service" ]] && install -m 0644 "$BUNDLE/kms/kms-api.service" /etc/systemd/system/ || true
install -m 0644 "$NODE_SETUP_DIR/aastar-node-setup.service" /etc/systemd/system/ 2>/dev/null || \
  curl -fsSL "https://raw.githubusercontent.com/$REPO/main/kms/node-setup/aastar-node-setup.service" -o /etc/systemd/system/aastar-node-setup.service
# 首启:开 KMS provisioning(向导要 /gen-key)
mkdir -p /etc/systemd/system/kms-api.service.d
printf '[Service]\nEnvironment=KMS_BLS_PROVISIONING=1\n' > /etc/systemd/system/kms-api.service.d/prov.conf

# ── 6. 起服务 ────────────────────────────────────────────────────────
log "重载 + 重启 tee-supplicant + 起 KMS + 向导"
systemctl daemon-reload
systemctl restart "tee-supplicant@teepriv0.service" 2>/dev/null || systemctl restart tee-supplicant 2>/dev/null || true
sleep 2
systemctl enable --now kms-api.service 2>/dev/null || true
# 只有还没配置时才起向导(幂等)
if [[ ! -f "$CONFIG_DIR/kms.env" ]]; then
  systemctl enable --now aastar-node-setup.service 2>/dev/null || true
fi

IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
log "✅ 完成。"
echo "   KMS:  http://$IP:3000/health"
if [[ ! -f "$CONFIG_DIR/kms.env" ]]; then
  echo "   配置向导: http://$IP:8088  (setup token 见: journalctl -u aastar-node-setup)"
else
  echo "   已配置过(存在 $CONFIG_DIR/kms.env)。"
fi
