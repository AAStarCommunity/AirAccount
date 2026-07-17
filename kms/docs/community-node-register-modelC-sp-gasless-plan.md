<!-- Created: 2026-07-17 -->
# 模型 C — SuperPaymaster ERC-4337 gasless 社区节点注册（最终形态规划）

> **派发对象**：`repo:superpaymaster`（SP 合约 + paymaster）、`repo:sdk`（aastar-sdk）
> **发起**：`repo:airaccount`
> **性质**：**规划 / RFC**（非详细实现）。这是 jason 期望的**最终流程**——社区**零资产、零 gas**完成节点注册。
> **状态**：目标 + 约束 + 待 SP/SDK 设计的开放问题（本文）。

---

## 1. 目标（终态一句话）

社区买板 → 开机填表 → **不持有任何 ETH / GToken** → 一键完成 `registerRole + registerWithProof` → 成为门限网络独立节点。**gas 与质押全部由生态承担/垫付。**

对比：模型 A（预充值，AirAccount 在做）与模型 B（AAstar 代付服务，派 DVT 仓）都要 AAstar **实际出 GToken 质押**。模型 C 追求把这件事**协议化、可持续**（不是靠运营方手工掏钱）。

---

## 2. 核心难点：gas ≠ stake

`registerWithProof` 要求 operator **先质押 30 GToken**（`requireStake=true`）。

- **Gas** 可由 **SP paymaster** 赞助（ERC-4337 标准能力）。
- **Stake（30 GToken）** 是**锁仓资产**，paymaster **管不了**。这是模型 C 相对 A/B 真正要解决的新问题。

---

## 3. 两部分，分别给 SP / SDK

### 3.1 Gas 赞助（相对成熟）
把 operator 的 `registerRole(ROLE_DVT)` + `registerWithProof(...)` 变成 **ERC-4337 UserOp**，由 **SP paymaster** 赞助 gas。

**待 SP/SDK 确认**：
- operator 是 **EOA**（现状：registerWithProof 用 `msg.sender`）。ERC-4337 赞助要求发起方是**智能账户(AA)**。是否让社区 operator 用 **AirAccount 智能账户**（本生态正好有 TEE+AA），把注册走成 AA 的 UserOp？→ 与 AirAccount 账户体系天然契合。
- 或 SP 提供一条"赞助 EOA 直发"的旁路（如 relayer 代广播 + paymaster 结算）。

### 3.2 Stake 垫付（模型 C 的关键创新）
30 GToken 质押谁出、怎么可持续。**请 SP 团队设计**，候选机制：

| 候选 | 机制 | 复用 SP v5 现成能力 |
|---|---|---|
| **a. 信用/债务垫付** | SP 用 **credit 系统**给新节点垫 30 GToken 质押，节点后续出块/签名收益**自动还债** | ✅ SP v5 已有 `recordDebt`/`repayDebt`/`getCreditLimit`/`getDebt`（见 INTERFACES 信用/债务分组） |
| **b. Sponsored stake** | 生态国库/SP 合约**代持**质押，节点仅获得"注册资格"，退出时归还 | 需新合约语义 |
| **c. 委托质押** | 质押来自国库池，绑定到节点 operator，slash 时从池扣 | 与 BLS slash 机制（`executeSlashWithBLS`）联动 |

**倾向**：优先评估 **a. 信用垫付**——SP v5 的 `recordDebt`/信用额度系统看起来正好能表达"先垫 30 GToken 质押、收益还债"，可能零新合约或最小改动。

### 3.3 SDK 侧
- `onboardDvtNode` 现在只有 `funderWallet`（模型 A/B 的直接转账）路径。**新增 gasless 路径**：
  - 组装 registerRole/registerWithProof 为 UserOp + `paymasterAndData`（SP 赞助）。
  - 对接 §3.2 选定的 stake 垫付机制（如信用系统的调用）。
- 与现有 `dvtOperatorActions.registerWithProof` / `buildDvtPop` / KMS `/pop` popSigner 接缝兼容。

---

## 4. 端到端设想流程（待细化）

```
板子(社区,零资产)
  ├─ KMS /gen-key(BLS in TEE) + /pop(PoP)
  ├─ operator = AirAccount 智能账户(TEE 保护)          ← 与 3.1 契合
  ├─ SDK 组装 UserOp: [信用垫付30 GToken 质押] + registerRole + registerWithProof
  │     paymasterAndData = SP 赞助 gas
  └─ 提交 → SP paymaster 付 gas + 信用系统垫质押 → 节点上链, 债务挂账
        节点后续签名/出块收益 → repayDebt 自动还清垫付
```

---

## 5. 开放问题（请 SP + SDK 讨论）
1. operator 用 **AA 智能账户** 还是保留 EOA + relayer 旁路？（影响 registerWithProof 的 msg.sender 语义）
2. stake 垫付选 **信用/sponsored/委托** 哪条？信用系统能否直接表达"垫质押 + 收益还债"？
3. 还债来源：节点收益如何计量与归集？无收益/退出/被 slash 时垫付如何清算？
4. 风控：谁有资格获得垫付（准入/PoP/声誉 SBT）？防女巫。
5. 与模型 B 的关系：C 落地前，B（运营方直接代付）作为过渡；C 成熟后 B 可退役。

---

## 6. 分工建议
- **`repo:superpaymaster`**：§3.1 paymaster 赞助注册 UserOp + §3.2 stake 垫付机制（优先评估信用系统）合约设计。
- **`repo:sdk`**：`onboardDvtNode` 新增 gasless 路径 + UserOp 组装 + 对接 SP 赞助/垫付。
- **`repo:airaccount`**：提供 KMS `/pop`（已有）+ 板侧向导对接；operator 若用 AA 账户，联动 AirAccount 账户体系。

> 这是最终形态，优先级低于模型 A（先上线）/B（过渡）。本文作为 RFC 供 SP/SDK 排期与设计。
