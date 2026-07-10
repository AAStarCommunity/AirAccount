# AirAccount KMS — 测试网 → 生产切换清单

> 核心认知：**passkey rpId 恒为 `aastar.io`，测试网与主网必须一致**（passkey 绑定在 rpId 上，
> 换 rpId = 作废所有已注册 passkey）。所以"切换"切的**不是 rpId**，而是三样东西：
> **构建 flag（去 dev-rpid）→ 合约 config（contracts.active）→ RPC/secrets**。代码逻辑零差异。
>
> 关联：`deploy-runbook-3node.md`（3 节点部署序列）、CC-30（发布盘点）、CC-24（imx93 KMS-TEE 无人值守）。

---

## 0. 一图流

```
测试网镜像 (dev-rpid, contracts.active=testnet, Sepolia RPC)
        │  ① 重编生产镜像 (去 dev-rpid, 留 strict-challenge)
        │  ② contracts.active=mainnet + 回填 OP 主网地址
        │  ③ RPC/secrets 换生产 + provision api key(fail-closed)
        │  ④ 主网前硬件根 #99/#50/#127/#128 一趟 TA 重刷
        │  ⑤ 上链 registerBLSPublicKey 2-of-3
        ▼
生产镜像 (rpId=aastar.io only, contracts.active=mainnet)
```

---

## 1. 构建：从 dev/transition 镜像 → 生产镜像

当前板（kms.aastar.io / .59）是 **`dev-rpid` 构建**——它把 `localhost` 塞进 WebAuthn
rpId/origin 默认值，启动打印 `⚠️ DEV-RPID build … NOT a production image`
（`kms/host/src/api_server.rs:1108`）。生产镜像必须去掉它。

| 项 | 测试/过渡镜像 | 生产镜像 |
|---|---|---|
| cargo feature `dev-rpid`（CA）+ TA `dev-rpid` | **开** → 接受 localhost rpId/origin | **关** → 仅 `aastar.io` / `https://kms.aastar.io` |
| cargo feature `strict-challenge`（CA）+ TA `strict-challenge` | 开 | **开**（保留） |
| `KMS_RP_ID` | 默认含 localhost | `aastar.io` |
| `KMS_ORIGIN` | 默认含 `http://localhost:*` | `https://kms.aastar.io` |
| passkey/binding 导出（#150） | 未落地（无端点） | 确认仍未启用（TA 侧一并核） |

**构建命令**（去掉 `--features dev-rpid`，保留 `strict-challenge`）——TA + CA 都要重编，
见 `BUILD-MX93.md`。TA 侧的 `dev-rpid` feature 与 CA 侧配对，两边必须一致，否则
`/version` 会报 profile 不一致（见 `build_mx93_board_features` 约定）。

**验收**：`GET /version` 的 profile 应报 **prod**（不是 transition/dev），启动日志**不再**打印
DEV-RPID 警告；用 localhost origin 的 assertion 被拒。

---

## 2. 合约 config：contracts.active 一键切

`kms/deploy/topology-aastar-3node/` 的 node profile 里合约地址是 testnet/mainnet 双组，
改 `contracts.active = mainnet` 即切。**主网地址等各合约仓部署后回填**（CC-30 handoff）：

| 字段 | 来源仓 | 状态 |
|---|---|---|
| `validator`（DVT 算法合约） | **repo:dvt**（YetAnotherAA-Validator 部署；airaccount `ValidatorRouter.getAlgorithm(0x01)` 指向它） | 主网待部署（Sepolia = `0x539B9681…`） |
| `entry_point` | canonical EntryPoint v0.7 `0x0000000071727De…` | 两网同址 |
| `e2e_account` | **repo:airaccount-contract**（主网 impl 部署后 mint 回填 `contracts_mainnet.e2e_account`） | 主网待回填 |
| `aPNTs`（gasless） | **repo:sp** | 主网待回填 |

`ETH_RPC_URL` / `BUNDLER_RPC_URL` → OP 主网。

---

## 3. Secrets：全手动 provision，不入 config

| secret | 说明 |
|---|---|
| **API key（KMS_API_KEY…）** | ⚠️ **认证 fail-closed（#145）——不 provision 会 brick**，切换前**必须先 provision** |
| keeper/operator EOA（CC-34） | `KMS_KEEPER_PROVISIONING=1` 跑一次 `gen-keeper-eoa` → 记 `KMS_KEEPER_KEY_ID` / `KMS_KEEPER_ADDRESS`（充值 EOA），关闭 provisioning gate |
| BLS signer / keeper signer token | `KMS_BLS_SIGNER_TOKEN` / `KMS_KEEPER_SIGNER_TOKEN`（loopback :3100 gate） |
| tunnel token / keystore 密码 | 手动，只进 tmpfs（见 CC-24 无人值守设计） |

---

## 4. 主网前硬件根（一趟 TA 重刷，~1 月观察期后）

CC-30 盘点的 4 个主网硬阻塞，**必须在主网前补齐**（非测试网阻塞）：

- **#99** 硬件安全根基（RPMB + secure boot + strict flip）
- **#50** RPMB 防回滚 key 编程
- **#127** 主网前最终安全复审（对抗审查）
- **#128** 生产密钥保管 + 事故响应预案

---

## 5. 上链注册 + 验收

1. 3 板 provision 各自 TEE BLS/keeper pubkey → 上链 `registerBLSPublicKey`，2-of-3 门限。
2. `GET /version` profile=prod、`GET /health` 绿、`/RollbackCounter` 正常。
3. config 驱动 E2E（`run-full-e2e.sh` 指向主网 config）。
4. 测试网正式版跑满 **~1 个月观察期** → 再切主网（代码零改动，纯 config）。

---

## 附：为什么 rpId 不在"切换"清单里

passkey 是 FIDO2 凭据，绑定在 rpId 域名上。若测试网用 `test.aastar.io`、主网用 `aastar.io`，
用户测试网注册的 passkey 到主网**全部失效**、需重新注册 + social recovery。所以两网**共用**
`aastar.io` rpId（origin `https://kms.aastar.io`），从测试网第一天就用生产 rpId——这正是
"测试=生产一致"的根因（见 `decentralization_model`：passkey rpId 是半去中心化的域名锚点）。
