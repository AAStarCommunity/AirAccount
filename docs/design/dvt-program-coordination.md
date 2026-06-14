# DVT program 协调记录（跨仓库分工 · 领取 / 交付 / 核对）

> 创建时间：2026-06-14 20:10 +07（本机时间）
> 关联：`dvt-solution.md`（设计）· `threat-model-ca-adversary.md` V5 · AirAccount #70
> **协调 hub（单一事实源 + 进度表）：AAStarCommunity/YetAnotherAA-Validator#42**

## 为什么需要协调

DVT（#70）**不是一个仓库一个 PR 能完成的 contained feature**，是跨多仓库的 program。本文档把分工、依赖、落地顺序、核对机制**记录在案**，让每个仓库**按自己的约定/文档**领取任务、交付、并被跨仓库核对。

## 架构定位（一处常见误解的澄清）

yet-dvt（YetAnotherAA-Validator）是 **program hub + BLS 共签节点实现**，**不是"统领/控制"其它仓库**——各仓库是**协作**关系，各自拥有自己那一片。运行时真正的「汇聚者」是 **aastar-sdk（客户端收集各方签名）+ 链上 account/SuperPaymaster（验门限）**。

## 分工与 issue 映射

| 仓库 | 任务 | issue | 依赖 |
|---|---|---|---|
| YetAnotherAA-Validator | DVT 节点软件（BLS 门限共签 + 独立策略 + 独立通道）+ program hub | #42 | — |
| SuperPaymaster | ROLE_DVT 合约（注册/质押/退出/slash + BLS 注册/聚合验证）| #283 | — |
| airaccount-contract | 智能账户链上验组合签名（KMS 主签 + ≥门限 DVT BLS 聚合）| #110 | SuperPaymaster#283 |
| **AirAccount（本仓库）** | KMS 主签 + 大额 op 需 ≥门限 DVT 共签的集成闸门 | #70 | 节点协议(#42) |
| aastar-sdk | 客户端聚合（收 KMS 主签 + DVT BLS 部分签 → 聚合 → 提交，独立通道）| #63 | #283 / #42 |
| Brood (PGL) | 激励：贡献 SBT 绑「正确执行策略」非签名次数 | #3 | #283 |

## 落地顺序（建议）

1. **协议先定**（yet-dvt #42）：BLS12-381 聚合格式 + 策略接口 + 独立确认通道。
2. **链上基础**：SuperPaymaster #283（ROLE_DVT / BLS 验证）→ airaccount-contract #110（验组合签名）。
3. **集成 + 运行时**：AirAccount #70（大额门限闸门）+ aastar-sdk #63（聚合提交）。
4. **激励闭环**：Brood #3（PGL SBT）。
5. **co-signer 角色（B）先行**（不卡 R-1，即使假 TEE 也缓解 V5）；验证者角色（A）依赖 #37 / R-1，靠后。

## 核对机制（每个交付都要过）

- **单一事实源**：yet-dvt #42 的进度表；各仓库 issue 链回 hub。
- **各仓库自治**：按自己 CI / 约定 / 文档实现 + 自测 + 交付。
- **跨仓库核对**：接口对齐 + 聚合签名端到端能验通（节点出部分签 → SDK 聚合 → 链上验过）。
- **命门复核（不可省）**：① **独立性**（独立 BLS key + CA 改不了的策略 + 不经 CA 的通道）② **激励不绑签名次数**。任一不满足，DVT 退化为橡皮图章，安全增益归零。

## 谁来协调

持久协调载体 = **yet-dvt #42 进度表 + 各仓库 issue**（不依赖任何人/agent 在线）。跨仓库推进由 **yet-dvt 仓库 owner / jason** 驱动；AI 助手按需在单次会话内协助**核对**（接口对齐、命门复核），不是常驻协调者。

## 本仓库（AirAccount）的那一块

AirAccount #70：KMS 继续做**主签名方**；对**大额/高风险 op**，在签名流程里要求并校验 **≥门限个独立 DVT 节点的 BLS 聚合共签**才放行。co-signer 不卡 R-1，可在 #42 节点协议定型后先行集成。
