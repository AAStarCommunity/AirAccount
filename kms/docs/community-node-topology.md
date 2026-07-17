# AAStar 社区节点拓扑（当前阶段架构 · KMS↔DVT 协作基础）

> 本文件是**当前阶段**（2026-07）KMS + DVT 联合部署的权威架构 / 配置约定，
> 作为 kms ↔ dvt 跨仓协作的基础。变更需同步 CC-30 / CC-22 / CC-24 / CC-34。
> 关联：`deploy-runbook-3node.md`、`testnet-to-prod-switch.md`、`dvt-tee-bls-custody-design.md`。

---

## 1. 物理设备（3 块板）

| 板子 | 位置 | 架构 | RAM | 备注 |
|---|---|---|---|---|
| **MX93-A** | 机房（图书馆） | arm64 | 2GB | **当前连本 Mac 的板** = 生产 KMS + dvt.aastar.io |
| **DK2** | 机房 | armv7 (32-bit) | 512MB | STM32MP157F |
| **MX93-B** | 家 | arm64 | 2GB | **到货中**（新板），到货后邮寄/带回家 |

网络：机房 = **图书馆网络**（A 板与 Mac 同网）。家 = 家庭宽带。

---

## 2. 生产环境（主网 + 测试网，同码不同 config）

> 唯一区别 = 配置（RPC / 合约地址 / env），代码零差异。见 `testnet-to-prod-switch.md`。

| 角色 | 跑在哪块板 | BLS 签名模式 | DVT 部署路径 | 状态 |
|---|---|---|---|---|
| **KMS 生产** | 机房 MX93-A | TEE (OP-TEE) | — | 主网+测试网都在这 |
| **DVT1** | 机房 MX93-A（与 KMS 同板 co-located） | **KMS-TEE 托管** `RUST_SIGNER_URL=http://127.0.0.1:3100` | `deploy/imx93`（co-located） | CC-22/CC-24 ✅ |
| **DVT2** | 机房 DK2 | 本地 EIP-2335 keystore | `deploy/dk2` | CC-32 ✅ PR #206 |
| **DVT3** | **家 MX93-B** | 本地 EIP-2335 keystore | `deploy/imx93`（DVT-only） | 路径就绪 ✅ |

→ **DVT1/2/3 组 2-of-3 门限**（容忍 1 挂），**1 个生产 KMS**（机房 A 板）。
→ 地理分布：2 机房（A:DVT1 + DK2:DVT2）+ 1 家（B:DVT3）。geo-diversity 有限，testnet 可接受；主网前评估异地（见 #128 社恢复兜底）。

---

## 3. 测试/开发环境

| 角色 | 跑在哪 | 说明 |
|---|---|---|
| **KMS 测试** | 家 MX93-B | 家里那块板**兼作**测试 KMS |
| DVT1/2/3（虚拟） | 本地进程 Mock TEE | 端口 3001-3003，不落真板 |

> ⚠️ **家 MX93-B 是双角色**：稳定后**同时跑「测试 KMS + 生产 DVT3」**。
> 测试 KMS 用独立 keyspace/DB，与其上跑的生产 DVT3 隔离；DVT3 是 live 2-of-3 的一员。

---

## 4. 板子命名与流转（重要，避免混淆）

- **A 板** = 当前连本 Mac 的板 = 机房生产（KMS 生产 + DVT1 + dvt.aastar.io）。**当前主板**。
- **B 板** = 到货中的新板 → 带回**家** → 稳定后跑「测试 KMS + 生产 DVT3」。
- **DK2** = 机房，跑生产 DVT2（armv7）。

---

## 5. KMS↔DVT 协作约定（跨仓契约）

| 契约 | 值 / 位置 | 归属 |
|---|---|---|
| BLS 托管 signer | `POST 127.0.0.1:3100/sign`（EIP-2537 256B / DST `_POP_`） | kms 实现，dvt 消费（`RUST_SIGNER_URL`） |
| keeper/operator ECDSA（CC-34） | `POST 127.0.0.1:3100/kms/sign`（65B r‖s‖v）+ `/kms/gen-keeper-eoa` | kms 实现，dvt 消费（`KEEPER_SIGNER_URL`） |
| signer token | `KMS_BLS_SIGNER_TOKEN`（BLS，可选）/ `KMS_KEEPER_SIGNER_TOKEN`（keeper，**必设 fail-closed**） | 两仓共享密钥 |
| validator（DVT 算法合约） | Sepolia `0x539B9681aFd5BFbCaa655Fe4c6BdcFe1fa7864bC` | **repo:dvt 部署**；airaccount `ValidatorRouter.getAlgorithm(0x01)` 指向它 |
| DVT bundle pin | v1.11.0（稳定版） | dvt 发布，kms 打进 airaccount-node bundle |
| owner-auth 接口 | `isValidOwnerAuth→0xa0cf00cf`（INTERFACES.md，CC-23） | airaccount-contract |

WiFi 凭据 / tunnel token / keystore 密码：**全本地 provision，不入协同中枢 / 不入 github**。

---

## 6. 目标：加电自运行的社区节点（自启动 + 首次自动初始化 + 幂等）

**诉求**：一块板加电 → KMS + DVT 自动起来 → 首次自动初始化（生成密钥/注册）→ 之后每次加电幂等自启，无需人工干预。

**幂等自启动的 4 个不变量**：
1. **密钥**：TEE 内 BLS/keeper key 缺失才生成（singleton，已存在不覆盖）；本地 keystore 缺失才生成。
2. **链上注册**：pubkey 未注册才 `registerBLSPublicKey`；已注册跳过。
3. **服务顺序**：`kms-api.service` 先起 → `dvt.service`（`After=kms-api.service`）；DVT key-less 抢先起也安静待命。
4. **秘密**：token / keystore 密码首次生成并落到位（tmpfs 密码 = 断电取舍，见 CC-24）；已存在复用。

> 详细设计 + 实现见（待补）`community-node-selfboot.md` / `node-setup` 向导。当前阻塞：生产 imx93 板到货走真机。

## 7. 🔴 主网 keeper 充值前硬门（承重）

CC-34 keeper 签名（PR #168 已 merge `03c7536`）在 co-located 板落地，但**给 keeper EOA 充真钱前**必须先过：

- **`/dev/tee*` device-auth**：host 的 `KMS_KEEPER_SIGNER_TOKEN` 只挡 loopback HTTP；**任何能访问 `/dev/tee*` 设备的进程可绕过 host、直接调 TA `KeeperSign`** → 对充值 EOA 即时可盗。这是与 BLS 同的 TEE 信任边界（非缺陷），但因是钱 key，主网充值前必须 **systemd 把 `/dev/tee*` 设备权限锁定给 KMS 服务专属**（`DeviceAllow` + 独立 user + 其余进程无设备访问）。
- 配合本仓 `#99/#50/#127/#128`（RPMB + secure boot + strict flip + 密钥保管预案）一趟 TA 重刷。
- 测试网 keeper（B 板调试台）无需此硬门；仅**主网充真钱前**。
