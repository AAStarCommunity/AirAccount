# DVT program 协调记录（跨仓库分工 · 领取 / 交付 / 核对）

> 创建时间：2026-06-14 20:10 +07（本机时间）
> 修订：2026-06-15 +07 —— 改正 #70 KMS 侧职责（见下「⚠️ 改正」）
> 关联：`dvt-solution.md`（设计）· `threat-model-ca-adversary.md` V5 · AirAccount #70
> **协调 hub（单一事实源 + 进度表）：AAStarCommunity/YetAnotherAA-Validator#42**

## ⚠️ 改正（2026-06-15）：KMS 不参与 BLS，#70 KMS 侧无代码

本文档初稿把 AirAccount #70 写成「KMS 要求**并校验** ≥门限 DVT BLS 聚合共签」——**这是错的**，特此改正：

- **BLS 的部分签（DVT 节点）、聚合（aastar-sdk）、验证（airaccount-contract 链上账户）全在 KMS 之外。** KMS 只产出 secp256k1 主签（签的就是 `userOpHash`，C1 天然满足），**不签 BLS、不验 BLS、不打包组合签名**。
- **威胁模型上 KMS 来"把关"本就站不住**：KMS 跑在 CA 控制的宿主里，正是 DVT 要防的 V5 信任域；强制若放在 KMS，被攻陷的 CA 跳过自己这道闸门即可，形同虚设。**强制必须在链上 account 合约 + 独立 DVT 节点**。
- **结论**：#70 的 **co-signer 角色对 KMS 是零代码**（维持现状即可）；**验证者角色**卡 R-1（B线），现阶段做不了。DVT 这个 feature 真正落在 `airaccount-contract(#110) + DVT 节点(#42) + aastar-sdk(#63) + SuperPaymaster(#283)`。
- 组合签名的 wire 打包由 **aastar-sdk** 完成（它持有 KMS 主签 + DVT 聚合两段）、由 **airaccount-contract** 验证——**不是 KMS 的产出物**。故下文「airaccount-contract 依赖 #70 的组合签名格式」一并作废。

## 为什么需要协调

DVT（#70）**不是一个仓库一个 PR 能完成的 contained feature**，是跨多仓库的 program。本文档把分工、依赖、落地顺序、核对机制**记录在案**，让每个仓库**按自己的约定/文档**领取任务、交付、并被跨仓库核对。

## 架构定位（一处常见误解的澄清）

yet-dvt（YetAnotherAA-Validator）是 **program hub + BLS 共签节点实现**，**不是"统领/控制"其它仓库**——各仓库是**协作**关系，各自拥有自己那一片。运行时真正的「汇聚者」是 **aastar-sdk（客户端收集各方签名）+ 链上 account/SuperPaymaster（验门限）**。

## 分工与 issue 映射（双向依赖）

| 仓库 | 任务 | issue | 它依赖谁（上游）| 谁依赖它（下游）|
|---|---|---|---|---|
| YetAnotherAA-Validator | DVT 节点软件（BLS 门限共签 + 独立策略 + 独立通道）+ program hub | #42 | —（**根，无上游**）| #283 #70 #110 #63 |
| SuperPaymaster | ROLE_DVT 合约（注册/质押/退出/slash + BLS 注册/聚合验证）| #283 | #42（BLS 方案一致）| #110 #70 #63 #3 |
| **AirAccount（本仓库）** | KMS 维持 secp256k1 主签（签 `userOpHash`）。**co-signer 对 KMS 零代码**（强制在链上）；验证者角色卡 R-1 | #70 | —（KMS 主签已有）| —（不被任何 DVT 环节依赖）|
| airaccount-contract | 智能账户链上验组合签名（KMS 主签 + ≥门限 DVT BLS 聚合）+ **强制大额需 DVT** | #110 | #42 #283 | —（终端：链上验证 + 强制）|
| aastar-sdk | 客户端聚合（收 KMS 主签 + DVT BLS 部分签 → 聚合 → **打包组合签名** → 提交，独立通道）| #63 | #42 #283 | —（终端：运行时）|
| Brood (PGL) | 激励：贡献 SBT 绑「正确执行策略」非签名次数 | #3 | #283 | —（终端：经济闭环）|

### 依赖链（一图看全）
```
        #42 节点协议(BLS方案/策略/独立通道)   ← 一切的根
          │
          ▼
        #283 ROLE_DVT 合约(BLS链上验/注册/slash)
          │            │
   ┌──────┴──────┐     └────────────► #3 PGL 激励(并行,#283 后可起)
   ▼             ▼
 #110 链上验组合签名 + 强制大额需DVT   #63 SDK 聚合+打包组合签名(收 KMS主签 + 节点部分签)
   （终端：链上强制）                    （终端：运行时汇聚）

 #70 KMS：维持 secp256k1 主签(签 userOpHash)，不在依赖链上——co-signer 对 KMS 零代码
```

## 落地顺序（建议·严格按依赖链）

1. **#42 协议先定**：BLS12-381 聚合格式 + 策略接口 + 独立确认通道。**根，必须最先。**
2. **#283 ROLE_DVT 合约**：BLS 方案与 #42 一致。
3. **#110 链上验组合签名 + 强制大额需 DVT**（依赖 #42+#283）。**强制全在此**，不在 KMS。
4. **#63 SDK 聚合 + 打包组合签名**：收 KMS secp256k1 主签 + #42 部分签 → 聚合 → 打包 → 提交，**最后接**。
   - 注：#70 KMS 维持现状即可（secp256k1 主签已有），不在此关键路径上。
5. **#3 PGL 激励**：#283 之后即可起，可与 3/4 并行。
- **横切**：**co-signer 角色（B）先行**（不卡 R-1，即使假 TEE 也缓解 V5）；验证者角色（A）依赖 #37 / R-1，靠后。

## 核对机制（每个交付都要过）

- **单一事实源**：yet-dvt #42 的进度表；各仓库 issue 链回 hub。
- **各仓库自治**：按自己 CI / 约定 / 文档实现 + 自测 + 交付。
- **跨仓库核对**：接口对齐 + 聚合签名端到端能验通（节点出部分签 → SDK 聚合 → 链上验过）。
- **命门复核（不可省）**：① **独立性**（独立 BLS key + CA 改不了的策略 + 不经 CA 的通道）② **激励不绑签名次数**。任一不满足，DVT 退化为橡皮图章，安全增益归零。

## 谁来协调

持久协调载体 = **yet-dvt #42 进度表 + 各仓库 issue**（不依赖任何人/agent 在线）。跨仓库推进由 **yet-dvt 仓库 owner / jason** 驱动；AI 助手按需在单次会话内协助**核对**（接口对齐、命门复核），不是常驻协调者。

## 本仓库（AirAccount）的那一块

AirAccount #70：KMS 继续做 **secp256k1 主签名方**（签 `userOpHash`）。**co-signer 对 KMS 零代码**——大额需 DVT 的**强制与校验全在链上 account 合约（#110）+ 独立 DVT 节点（#42）**，KMS 不签/不验/不打包 BLS（理由见顶部「⚠️ 改正」：KMS 在 CA 信任域内，自己把关形同虚设）。验证者角色（独立验 passkey/attestation）卡 R-1，属 B线，现阶段做不了。
