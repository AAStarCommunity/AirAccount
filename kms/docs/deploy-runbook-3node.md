<!-- Created: 2026-07-09 -->
# AAStar 3-node 部署 runbook（2-of-3 DVT + 1 KMS）

配套配置：`kms/deploy/topology-aastar-3node/`（README + 3 份 node profile）。
测试网 → 主网**只差配置**（`contracts.active` + `[chain]` RPC），代码零改动。

## 0. 拓扑

| 节点 | 设备 | 位置 | 角色 | 架构 | profile |
|---|---|---|---|---|---|
| node1 | MX93（新） | 学校机房 | **KMS 生产** + DVT1（TEE 托管） | arm64/2GB | `node1-school-mx93.toml` |
| node2 | **DK2** | 学校机房 | DVT2（独立） | **armv7/512MB** | `node2-school-dk2.toml` |
| node3 | MX93（当前主板） | 家里公寓 | DVT3（独立） + **KMS 测试** | arm64/2GB | `node3-home-mx93.toml` |

DVT 门限 **2-of-3**（容忍 1 挂）。家里 node3 略不稳 = 少数派，可接受。

## 1. 生产部署（设备到货后）

### node1 · 学校 MX93 · KMS 生产 + DVT1（TEE 托管）
```bash
# 1) 刷板 + 网络：照 kms1 方案（Cloudflare tunnel + 学校 WiFi），profile 填 board_ssh/domain
# 2) 部 KMS 生产（TEE，RSA-4096 TA signing，见 kms/scripts）
kms/scripts/deploy.sh
# 3) 部 DVT1，BLS 私钥托管进 KMS TEE（Variant B / CC-24）
kms/deploy/deploy-dvt.sh --config kms/deploy/topology-aastar-3node/node1-school-mx93.toml \
                         --dvt-repo ~/Dev/aastar/YetAnotherAA-Validator
#    DVT env 注入：RUST_SIGNER_URL=http://127.0.0.1:3100  RUST_SIGNER_REQUIRED=true
#                  RUST_SIGNER_TOKEN=<KMS_BLS_SIGNER_TOKEN>
# 4) 迁移：KMS gen-key 出新 BLS pubkey → 链上 registerBLSPublicKey → 切 RUST_SIGNER_URL
```

### node2 · 学校 DK2 · DVT2（独立，armv7）
> ⚠️ **等 @repo:dvt 出 DK2 armv7 部署脚本**（协同任务已 @，`deploy-dvt.sh` 硬编码 arm64 跑不了 DK2）。
```bash
# 前置：CMU IoT 门户注册 DK2 MAC 24:cd:8d:4e:4f:28 → provision wpa_supplicant
#       (SSID @JumboPlusIoT5GHz，PSK 见 gitignored .env.dk2-node2；同 KMS 学校网络方案)
# 部署：用 DVT 侧 armv7 脚本，DVT-only 独立模式（本地 EIP-2335 keystore，RUST_SIGNER_URL 不设）
#       512MB tuning：--max-old-space-size≈256 + zram/swap；npm build 可能需 host cross-build
```

### node3 · 家里 MX93 · DVT3（独立） + KMS 测试
```bash
# DVT3：deploy-dvt.sh 独立模式（arm64，复用 node1 路径，RUST_SIGNER_URL 不设）
kms/deploy/deploy-dvt.sh --config kms/deploy/topology-aastar-3node/node3-home-mx93.toml \
                         --dvt-repo ~/Dev/aastar/YetAnotherAA-Validator
# KMS 测试实例：独立 KMS_DB_PATH（如 /var/lib/kms-test/kms.db），绝不碰 node1 生产库
```

### 上链注册（2-of-3 生效）
每个 DVT 节点 `registerBLSPublicKey`（operator EOA 付 gas，须是 validator owner）。
node1 注册的是 **KMS TEE 生成的新 pubkey**；node2/node3 注册各自本地 keystore pubkey。

## 2. 测试环境（本地，条件受限版）

**虚拟 DVT1/2/3 = 直接用 DVT 仓已有的本地多节点**（不重造，见 DVT `README.md` §Multi-Node）：
```bash
cd ~/Dev/aastar/YetAnotherAA-Validator
NODE_STATE_FILE=./node_dev_001.json PORT=3001 npm run start:dev   # DVT1
NODE_STATE_FILE=./node_dev_002.json PORT=3002 npm run start:dev   # DVT2
NODE_STATE_FILE=./node_dev_003.json PORT=3003 npm run start:dev   # DVT3
# gossip：GOSSIP_BOOTSTRAP_PEERS=ws://localhost:3002/ws,ws://localhost:3003/ws
```

**KMS 测试侧**（本仓）：
- 快测：Mock-TEE KMS（开发模式），跑 `kms/test/run-full-e2e.sh 127.0.0.1:3000`（41/41）
- 贴近生产：复用 **node3（家 MX93）的 KMS 测试实例**（真 TEE，独立 keyspace）做 KMS 测试机 → 不重复买设备
- TEE 托管联调：DVT 本地节点 `RUST_SIGNER_URL=http://<kms-test>:3100` 指向 KMS 测试实例，验证 BLS 签名字节一致 + 链上 validate=0

## 3. 测试网 → 主网（只改配置）

```
1. contracts.active: testnet → mainnet
2. 填 [contracts_mainnet].validator / e2e_account（等生产合约部署，CC-30）
3. [chain]: chain_id 1 + 主网 RPC
4. 代码零改动，重新 deploy
```

## 4. 发布节奏

测试网正式版（0.28.1 → Sepolia config）→ 观察 ~1 月 → 补 #99/#50/#127/#128 → 主网。
