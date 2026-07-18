#!/usr/bin/env bash
# aastar-node-installer — 在【出厂 NXP FRDM-IMX93 OP-TEE Linux】上一条命令装我们的 KMS/DVT 组件。
# 社区上手 Phase 2 路径 A(见 kms/docs/phase2-image.md)。**不刷 Linux**——板子出厂系统人人相同,
# 只装 airaccount-node bundle(我们的组件)+ 选一种情况的 config。
#
#   curl -fsSL .../aastar-node-installer.sh | sudo PROFILE=combined-independent bash
#   curl -fsSL .../aastar-node-installer.sh | sudo PROFILE=dvt-only VERSION=v0.28.1 bash
#
# 4 种情况(PROFILE,见 kms/deploy/community-profiles/ + community-node-init-config-design.md):
#   kms-only              只 KMS(密钥服务)
#   dvt-only              只 DVT(本地 keystore 独立节点)
#   combined-tee          KMS+DVT,DVT 签名走 KMS-TEE(BLS+keeper 托管)
#   combined-independent  KMS+DVT 同板但各自独立(DVT 本地 keystore,keeper 走 env)  ← 默认
#
# 幂等:可重复跑(升级)。前提:出厂镜像已含 OP-TEE + tee-supplicant;有网;root。
set -euo pipefail

REPO="AAStarCommunity/AirAccount"
VERSION="${VERSION:-latest}"
DVT_REPO="AAStarCommunity/YetAnotherAA-Validator"
DVT_TAG="${DVT_TAG:-v1.10.0}"
TA_DIR="/lib/optee_armtz"
CA_DIR="/root/AirAccount/target/release"
NODE_SETUP_DIR="/opt/aastar/node-setup"
DVT_DIR="/opt/aastar/dvt"
CONFIG_DIR="/etc/airaccount"
UUID="4319f351-0b24-4097-b659-80ee4f824cdd"

log() { echo -e "\033[0;34m[installer]\033[0m $*"; }
die() { echo -e "\033[0;31m[installer] $*\033[0m" >&2; exit 1; }

# ── 0. PROFILE → 装什么(4 种情况)──────────────────────────────────────
PROFILE="${PROFILE:-combined-independent}"
case "$PROFILE" in
  kms-only)             INSTALL_KMS=1; INSTALL_DVT=0; DVT_SIGNER=none ;;
  dvt-only)             INSTALL_KMS=0; INSTALL_DVT=1; DVT_SIGNER=local ;;
  combined-tee)         INSTALL_KMS=1; INSTALL_DVT=1; DVT_SIGNER=tee ;;
  combined-independent) INSTALL_KMS=1; INSTALL_DVT=1; DVT_SIGNER=local ;;
  *) die "未知 PROFILE=$PROFILE(kms-only|dvt-only|combined-tee|combined-independent)" ;;
esac
log "情况: $PROFILE  (KMS=$INSTALL_KMS DVT=$INSTALL_DVT DVT签名=$DVT_SIGNER)"

[[ $EUID -eq 0 ]] || die "需 root(sudo)"
# DVT-only 无需 OP-TEE;有 KMS 的情况才检查 tee-supplicant。
if [[ $INSTALL_KMS == 1 ]]; then
  command -v tee-supplicant >/dev/null 2>&1 || ls /usr/sbin/tee-supplicant >/dev/null 2>&1 || \
    die "没检测到 OP-TEE(tee-supplicant)—— 出厂 NXP FRDM-IMX93 OP-TEE 镜像应自带"
fi
for c in curl tar; do command -v $c >/dev/null || die "缺 $c"; done

# DRY_RUN:只查前提 + 打印计划,不下载/不改动 —— 可在活板上安全验证。
DRY_RUN="${DRY_RUN:-0}"
if [[ $DRY_RUN == 1 ]]; then
  log "[dry-run] 计划: PROFILE=$PROFILE → 装 KMS=$INSTALL_KMS DVT=$INSTALL_DVT(DVT签名=$DVT_SIGNER)"
  log "[dry-run] 前提已过: root$([[ $INSTALL_KMS == 1 ]] && echo ' + OP-TEE/tee-supplicant') + curl/tar"
  [[ $INSTALL_KMS == 1 ]] && log "[dry-run]   会装: KMS TA/CA + node-setup 向导 + kms-api.service + 首启 provisioning gate"
  [[ $INSTALL_DVT == 1 ]] && log "[dry-run]   会装: DVT(clone $DVT_TAG + deploy-dvt.sh, 签名=$DVT_SIGNER)"
  log "[dry-run] 未下载、未改动系统,退出。"
  exit 0
fi

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

# ── 1. 下 airaccount-node release(我们的 bundle,~50MB,不是 Linux 镜像)──
if [[ "$VERSION" == "latest" ]]; then
  URL="https://github.com/$REPO/releases/latest/download/airaccount-node-v0.28.1.tar.gz"
else
  URL="https://github.com/$REPO/releases/download/airaccount-node-$VERSION/airaccount-node-$VERSION.tar.gz"
fi
log "下载 $URL"
curl -fSL "$URL" -o "$WORK/node.tgz" || die "下载失败(检查 VERSION / 网络)"
tar -xzf "$WORK/node.tgz" -C "$WORK"
BUNDLE="$(find "$WORK" -maxdepth 1 -type d -name 'airaccount-node-*')"
[[ -d "$BUNDLE" ]] || die "release 结构异常"

mkdir -p "$CONFIG_DIR"

# ── 2. 装 KMS(TA + CA + attestation manifest)—— 仅有 KMS 的情况 ────────
if [[ $INSTALL_KMS == 1 ]]; then
  [[ -d "$BUNDLE/kms" ]] || die "release 缺 kms/"
  log "装 KMS TA/CA"
  install -m 0444 "$BUNDLE/kms/$UUID.ta" "$TA_DIR/$UUID.ta"
  mkdir -p "$CA_DIR"; install -m 0755 "$BUNDLE/kms/kms-api-server" "$CA_DIR/kms-api-server"
  [[ -f "$BUNDLE/kms/attestation-measurements.json" ]] && \
    install -m 0444 "$BUNDLE/kms/attestation-measurements.json" "$CONFIG_DIR/attestation-measurements.json" || true
fi

# ── 3. 装 node-setup 向导(+ register bundle)── 有 KMS 的情况才起 web 向导 ─
log "装 aastar-node-setup 向导 + register 组件"
mkdir -p "$NODE_SETUP_DIR"
if [[ -d "$BUNDLE/node-setup" ]]; then cp -r "$BUNDLE/node-setup/." "$NODE_SETUP_DIR/"; else
  for f in setup-server.py setup.html register-node.mjs; do
    curl -fsSL "https://raw.githubusercontent.com/$REPO/main/kms/node-setup/$f" -o "$NODE_SETUP_DIR/$f" 2>/dev/null || true
  done
fi

# ── 4. 装 DVT ── 仅有 DVT 的情况;签名模式按 PROFILE ────────────────────
if [[ $INSTALL_DVT == 1 ]]; then
  if [[ ! -d "$DVT_DIR/.git" ]]; then
    log "clone DVT $DVT_TAG(NestJS + Rust signer,build 较慢)"
    git clone --depth 1 --branch "$DVT_TAG" "https://github.com/$DVT_REPO.git" "$DVT_DIR" || die "DVT clone 失败"
  fi
  # 签名模式:tee=DVT BLS 走 KMS-TEE(RUST_SIGNER_URL);local=本地 EIP-2335 keystore。
  if [[ $DVT_SIGNER == tee ]]; then
    export RUST_SIGNER_URL="http://127.0.0.1:3100"   # deploy-dvt.sh 读它→托管模式
    export DVT_KEEPER_MODE="kms"                       # keeper EOA 走 KMS TEE(CC-34)
    log "DVT 签名 = KMS-TEE 托管(RUST_SIGNER_URL=$RUST_SIGNER_URL)"
  else
    unset RUST_SIGNER_URL 2>/dev/null || true          # 不设=本地 keystore 独立模式
    export DVT_KEEPER_MODE="env"                        # keeper EOA 走 env/本地 key
    log "DVT 签名 = 本地 keystore 独立(keeper 走 env)"
  fi
  log "DVT 部署(交给 deploy-dvt.sh 处理 build/keystore/systemd)"
  if [[ -x "$DVT_DIR/deploy/deploy-dvt.sh" ]]; then
    ( cd "$DVT_DIR" && ./deploy/deploy-dvt.sh ) || log "⚠️ deploy-dvt.sh 返回非零,检查 DVT 日志"
  else
    log "⚠️ DVT deploy 脚本缺失,需手动 build"
  fi
fi

# ── 5. systemd units + 首启 provisioning ─────────────────────────────
log "装 systemd units"
if [[ $INSTALL_KMS == 1 ]]; then
  [[ -f "$BUNDLE/kms/kms-api.service" ]] && install -m 0644 "$BUNDLE/kms/kms-api.service" /etc/systemd/system/ || true
  # 首启:开 KMS provisioning(向导/selfinit 要 /gen-key);配置完 finalize 自动关。
  mkdir -p /etc/systemd/system/kms-api.service.d
  printf '[Service]\nEnvironment=KMS_BLS_PROVISIONING=1\n' > /etc/systemd/system/kms-api.service.d/prov.conf
fi
# web 向导:有 KMS 的情况才起(向导流程依赖 KMS /gen-key);dvt-only 用 deploy-dvt.sh 自带 provisioning。
if [[ $INSTALL_KMS == 1 ]]; then
  install -m 0644 "$NODE_SETUP_DIR/aastar-node-setup.service" /etc/systemd/system/ 2>/dev/null || \
    curl -fsSL "https://raw.githubusercontent.com/$REPO/main/kms/node-setup/aastar-node-setup.service" -o /etc/systemd/system/aastar-node-setup.service 2>/dev/null || true
fi

# ── 6. 起服务 ────────────────────────────────────────────────────────
log "重载 + 起服务"
systemctl daemon-reload
if [[ $INSTALL_KMS == 1 ]]; then
  systemctl restart "tee-supplicant@teepriv0.service" 2>/dev/null || systemctl restart tee-supplicant 2>/dev/null || true
  sleep 2
  systemctl enable --now kms-api.service 2>/dev/null || true
  if [[ ! -f "$CONFIG_DIR/kms.env" ]]; then
    systemctl enable --now aastar-node-setup.service 2>/dev/null || true
  fi
fi

IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
log "✅ 完成($PROFILE)。"
[[ $INSTALL_KMS == 1 ]] && echo "   KMS:  http://$IP:3000/health"
[[ $INSTALL_DVT == 1 ]] && echo "   DVT:  http://$IP:8080/health"
if [[ $INSTALL_KMS == 1 && ! -f "$CONFIG_DIR/kms.env" ]]; then
  echo "   配置向导: http://$IP:8088  (setup token: journalctl -u aastar-node-setup)"
fi
