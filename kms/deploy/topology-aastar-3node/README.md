<!-- Created: 2026-07-09 -->
# AAStar 3-node production topology (2-of-3 DVT + 1 KMS)

首套 Beta 生产 = 测试网正式版用同一套设备（条件受限，不额外买机）。DVT **2-of-3** 门限。

| 节点 | 设备 | 位置 | 角色 | KMS | DVT BLS 密钥来源 | profile |
|---|---|---|---|---|---|---|
| **node1** | MX93（新到） | 学校机房（稳，Mac Mini 接板） | **KMS 生产** + DVT1 | prod（TEE） | KMS TEE 托管（`RUST_SIGNER_URL=127.0.0.1:3100`） | `node1-school-mx93.toml` |
| **node2** | DK2 | 学校机房 | DVT2 | 无 | 本地 EIP-2335 keystore（独立） | `node2-school-dk2.toml` |
| **node3** | MX93（当前主板） | 家里公寓（略不稳，可接受） | DVT3 + **KMS 测试** | test（独立 DB/keyspace） | 本地 EIP-2335 keystore（独立） | `node3-home-mx93.toml` |

## 关键点

- **KMS 只有一份生产**（node1）。node3 的 KMS 是**测试机**，用独立 DB/keyspace，绝不与生产密钥混库（`KMS_DB_PATH` 分开）。
- **只有 node1 用 KMS TEE 托管 DVT BLS 私钥**（CC-24 Variant B，`RUST_SIGNER_URL` 指本机 :3100，私钥永不出 TEE）。node2/node3 的 DVT 用各自本地加密 keystore（独立模式，零 KMS 依赖）。
- **测试网 → 主网只差配置**：`contracts.active` 从 `testnet` 切 `mainnet` + 填 `[contracts_mainnet]` + 换 `[chain]` RPC。代码零改动。
- **board_ssh / domain / secrets 到货再填**：Tailscale IP 在刷板后分配；secrets 全部部署时手动 provision（见每份 toml 的 `[secrets]` 段）。

## 待补（阻塞项，非本仓）

- `[contracts_testnet].validator` 已填 v1.9.0 值（`0x539B96…`，repo:dvt 权威源 `deploy/sdk-dvt-config.testnet.json`）；主网值等生产合约部署（CC-30）。
- SP BLSAggregator 新地址等 `applyBLSAggregator`（SuperPaymaster #285 / CC-18）后同步。
- node2（DK2）是 **DVT-only** 节点：现有 `deploy-dvt.sh` 假设 KMS co-located（会校验 KMS 在位），DVT-only 部署路径需在 runbook 里用独立模式跑（见 `../../docs/kms-dvt-imx93-deploy.md` 拓扑 runbook 段）。

## 部署（到货后）

```bash
# node1（学校 MX93，KMS+DVT1）—— 先 KMS 后 DVT
kms/scripts/deploy.sh              # 部 KMS 生产（TEE）
kms/deploy/deploy-dvt.sh --config kms/deploy/topology-aastar-3node/node1-school-mx93.toml \
                         --dvt-repo ~/Dev/aastar/YetAnotherAA-Validator

# node2（学校 DK2，DVT2 独立）—— DVT-only，见 runbook 独立模式
# node3（家里 MX93，DVT3 + KMS 测试）—— KMS 测试实例用独立 KMS_DB_PATH
```
