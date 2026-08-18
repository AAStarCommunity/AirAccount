# /goal 指令 — CC-103 KMS 侧 committee 签名集成(零 TA 改动)

> 用法:把"## 交付契约"整段作为 `/goal` 的输入,无人值守跑到 ACCEPTANCE 全绿为止。
> 权威规范见同目录 `SPEC.md`;验收判据见 `ACCEPTANCE.md`。二者是本 goal 的前置约束,**先读**。

---

## 交付契约(/goal 输入)

**目标**:完成 AirAccount KMS 侧对 CC-98 committee BLS 签名变更的适配。**已定稿:对 KMS = 签名代码零改动**(KMS 是不透明签名机,详见 `docs/agent/cc103-kms-committee/SPEC.md`)。因此本 goal 不改 TA 签名逻辑,而是**用证据锁死"零改动是安全的"** + 补防漂移测试 + 落边界文档 + legacy 集成盘点。

**背景(权威,已由 airaccount-contract + dvt 逐字节确认)**:
- committee 只改 SDK/aggregator 组装的 per-signer wire(`nodeId‖slot‖merkleProof`)与账户注入的 accountId framing。
- **节点(含 co-located KMS-TEE)签的 BLS 原像 = `bytes(userOpHash)` 32B,committee 不改;域分离靠 DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_`,三方(blst/noble/validator)golden 一致。**
- accountId 绝不进签名(命门 B2),KMS 任何路径不产出 accountId,且**不在 TEE 加 accountId 绑定**。

**范围内(READY tasks)**:
1. **T1 B2 审计** — 静态审计所有签名路径(owner/keeper/session/bls_sign)证明零 accountId 构造/注入。产出:审计结论 + 证据命令(见 ACCEPTANCE §A)。
2. **T2 DST golden-vector 回归测试** — 新增 KAT 钉死 `bls_sign` 的 DST + 字节输出(ACCEPTANCE §B)。固定 (sk, userOpHash) → 固定签名字节;注释交叉引用 dvt golden 来源。**这是唯一新增代码**(测试,不碰 TA 逻辑)。
3. **T3 边界设计说明** — 新增 `kms/docs/committee-signing-boundary.md`,落 SPEC §4/§8(ACCEPTANCE §D)。
4. **T4 legacy 集成盘点** — 对新 Sepolia 地址(SPEC §7)静态盘点 legacy 路径可用;E2E 若需 testnet/keeper 则标 BLOCKED 记录(ACCEPTANCE §C)。

**范围外(不做,别越界)**:
- 不改 `kms/ta/src/bls.rs` 的签名/hash-to-curve 逻辑(committee 零改动;若发现"必须改 TA"→ **停下标 BLOCKED 说明,不擅自改**)。
- 不做 SDK 的 per-signer wire 组装(那是 repo:sdk)。
- 不碰 `TREE_DEPTH`/`requiredQuorum`/`committeeActive` 读取(KMS 不读)。
- 不翻 `setEpochLength`(dvt/CC-104)。

**完成定义**:`ACCEPTANCE.md` 的 A+B+C(静态)+D 全绿 + 现有测试无回归。达成即:更新 `docs/agent/cc103-kms-committee/progress.md`,并**准备好 PR**(自审 + 对抗 review 过关),回帖 CC-103(Seeder b8f3441f)更新 KMS 交付状态。

**硬约束(不可违反)**:
- 一 task 一分支一 PR;Feature 专属 worktree(本 goal 在 `../AirAccount-next` 或新开 `../AirAccount-<Fid>`)。
- 绝不 `git add -A`;绝不直推/合并到 main(走 PR + 外部评审 approve;作者 jhfnetboy 不能自 approve)。
- 绝不打印/存储私钥出 TEE;绝不绕 WebAuthn;绝不 `KMS_ALLOW_OPEN_MODE=1`。
- 板 CA 构建须带 `MX93_DEV_RPID=1 MX93_STRICT_CHALLENGE=1`。
- PR 前按 `grade-change` 定评审轮数自审 + codex/opus 对抗 review(见全局 review 流程)。
- 无人值守遇产品/架构未知 → 标 BLOCKED 记录,不猜、不替用户拍板。

**遇阻策略**:
- T4 E2E 缺 testnet/keeper → 标 BLOCKED,继续 T1/T2/T3(互不依赖)。
- T2 拿不到 dvt golden fixture → 先锁 KMS 自产 KAT + 挂"向 dvt 要 golden 做三方对齐"跟进项(followup ledger),不阻塞交付。
- 任何"似乎要改 TA"的信号 → 立即停,回到 SPEC 核对(大概率是误判,因为原像未变),仍不确定就 BLOCKED + 通知。

---

## 执行提示

- 起跑先 `Read SPEC.md + ACCEPTANCE.md`,再逐条推进 T1→T4(T1/T2/T3 可并行,T4 可能 BLOCKED)。
- 每完成一个 task 更新 `progress.md`;每个 task 一个分支一个 PR。
- 全部达成后 PushNotification 通知 + 回帖 CC-103。
- 这是**零 TA 改动**的低风险交付;主要产出是**测试 + 文档 + 审计证据**,不是改签名。若你发现自己在改签名逻辑,几乎一定是走偏了——回 SPEC §4。
