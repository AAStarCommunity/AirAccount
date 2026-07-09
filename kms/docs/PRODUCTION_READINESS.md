# AirAccount KMS — Production Readiness (CC-30)

> 关联协同任务：Cooperation-Center **CC-30**（YAAA 正式版发布 · 5 依赖仓盘点）
> repo 标签：`repo:kms`（= `repo:airaccount`；不含 `repo:airaccount-contract`）
> 最后更新：2026-07-09 · 当前版本：`airaccount-kms-v0.28.1` / `airaccount-node-v0.28.1`（Beta6）

## 0. 两条发布线（KMS 的"两个版本"）

| 发布线 | tag 前缀 | 含义 |
|---|---|---|
| KMS 独立 | `airaccount-kms-v*` | 只跑密钥/签名服务（独立 KMS 部署） |
| KMS+DVT 节点包 | `airaccount-node-v*` | 二合一 node bundle（co-located KMS+DVT） |

两条线当前同为 **0.28.1**。测试网 vs 主网**代码零差异**，唯一区别 = 配置（RPC / 合约地址 / env）。

## 1. Production-Readiness 表

标注：`两网只差配置` / `测试网就能发` / `主网前必须补`

| # | 项 | 评估 | 状态 | 阻塞? |
|---|---|---|---|---|
| 1 | 核心 KMS API（CreateKey/Sign/Grant/Revoke） | 逻辑完成，两网代码零差异 | ✅ | 两网只差配置 |
| 2 | WebAuthn strict challenge-binding | Beta3+ 完成，#49 nonce 绑定上板 | ✅ | 测试网就能发 |
| 3 | 认证 fail-closed（`KMS_ALLOW_OPEN_MODE`） | #145；⚠️部署前必须先 provision api key | ✅ | 测试网就能发 |
| 4 | Stats 页 XSS + UTF-8 panic | #144 已修 | ✅ | 测试网就能发 |
| 5 | E2E 全签名 op 一致性 | run-full-e2e 41/41；#68 payload-commitment 全覆盖 | ✅ | 测试网就能发 |
| 6 | 社区上手 Phase1/2/3 + 门户 | 0.28.1 上线 kms.aastar.io/portal | ✅ | 测试网就能发 |
| 7 | KMS+DVT 二合一基线（CC-22） | KMS 侧实测，2GB 单板 20 并发绰绰有余 | ✅ KMS侧 | 待 @repo:dvt 确认 build 路径 |
| 8 | Variant B：DVT BLS TEE 托管（CC-24） | PR#153 真机全绿，`:3100` 契约对齐 | ✅ KMS侧 | 待 @repo:dvt 轻接入 |
| 9 | #99 硬件安全根基（RPMB+secure boot+strict flip） | 代码就绪，差烧 key + 一次性重刷 | 🔴 | 主网前必须补 |
| 10 | #50 RPMB 防回滚 key 编程 | 一次性不可逆，含在 #99 一趟里 | 🔴 | 主网前必须补 |
| 11 | #127 主网前最终安全复审 | prod build 无 dev surface 对抗审查 | 🔴 | 主网前必须补 |
| 12 | #128 生产密钥保管 + 事故响应预案 | TA 签名 key 离线保管 + 泄漏预案 | 🔴 | 主网前必须补 |
| 13 | #122 CA/TA 一致性 CI 门 | 已挡 mint-op，盲点手工核 | 🟡 | 测试网就能发（非阻塞） |
| 14 | #37 远程证明 | Phase1 MVP 实机打通，Phase2 非阻塞 | 🟡 | 测试网就能发 |
| 15 | NXP NDA 厂商根 | 被拒（需法律实体），战略改「可复现+透明⊕DVT」 | ⚪ | 不阻塞（仅 V5 最高档） |

## 2. 依赖关系

**KMS 依赖别人（阻发布）：**
- `@repo:dvt` — 确认 v1.9.0 arm64 板上 build 路径（CC-22）+ TEE 托管轻接入（CC-24）+ 发 v1.10.0
- `@repo:sp` — BLSAggregator 地址 `applyBLSAggregator`（#285）、slash 流程（#139）
- `@repo:airaccount-contract` — `isValidOwnerAuth → 0xa0cf00cf` 已稳定；合并部署后一次性链上重注册 TEE 新 BLS pubkey
- `@repo:sdk` — Sepolia 地址同步（CC-12 / CC-19）

**别人依赖 KMS（已就绪）：**
- DVT ← KMS `127.0.0.1:3100` signer 契约（`POST /sign` → EIP-2537 256B）**已实现**
- YAAA ← 账户验证/签名
- airaccount-contract consumers ← `isValidOwnerAuth`
- docs ← 全部发完最后更新

## 3. 生产部署拓扑（本社区，条件受限版）

- **KMS：1 份**，跑在最稳的一块 NXP FRDM-IMX93 上（同板 co-located DVT#1）
- **DVT：3 个 = 2-of-3 门限**（容忍 1 个挂）
  - 新 MX93（学校机房）：KMS + DVT#1
  - DK2（学校机房）：DVT#2
  - 当前主板（家里）：DVT#3
- **测试环境**：本地虚拟（Mock TEE）跑 DVT1/2/3 + KMS；复用一份生产 KMS 做测试时用**独立 keyspace/DB**，不与生产密钥混库

**风险护栏：**
- KMS 单份 = 全系统可用性单点，且与 DVT#1 同板（相关性故障）。testnet 观察期可接受；主网前靠 #128 社恢复 + re-provision 预案兜底。
- 三块板同地点，无地理/供电/网络隔离 → 主网前评估至少 1 节点异地。

## 4. 发布路线

1. **测试网正式版（现在）**：0.28.1 配置指向 Sepolia → 部署 → CC-30 测试网就绪
2. **观察期 ~1 个月**：小修，跑真实流量
3. **主网前必须补**：#99 + #50 + #127 + #128 一趟 TA 重刷 + 对抗审查 + 密钥保管预案
4. **主网**：配置切主网 RPC/合约地址，代码零改动
