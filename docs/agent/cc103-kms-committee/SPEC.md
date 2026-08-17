# CC-103 committee BLS 签名 — KMS 侧权威规范(已定稿)

> 来源:协同任务 CC-103(Seeder b8f3441f),airaccount-contract + dvt 逐字节答复后**定稿**(2026-08-18)。
> 本文是 KMS 侧开发的**唯一权威参考**,GOAL/ACCEPTANCE 均以此为准。

## 0. 一句话结论

**CC-98 委员会 BLS 签名变更对 KMS = 签名代码零改动。** KMS 是"不透明签名机",committee 只改 SDK/aggregator 组装的 wire 与账户注入的 framing,**不改节点实际签的 BLS 原像**。KMS 交付 = 审计 + 回归测试 + legacy 集成 + 边界文档,**不动 TA**。

## 1. 背景

- CC-98 把 tier-2/3 BLS 从"whole-set 聚合"换成"per-proposal 委员会 + 随机抽样"。
- 账户侧 airaccount-contract **v0.31.0** 已部署 Sepolia(7/7 verified);dvt 验证器 **#237** 已合并 master。
- 剩余跨仓动作:SDK 产出新 per-signer wire;**KMS 经核实 = 零改动**。

## 2. 唯一权威 wire 变更(per-signer BLS 块) — 这是 **SDK/aggregator** 的活,不是 KMS

- **legacy(committeeActive 关,不变)**:`[ nodeIdsLength(32) ][ nodeId(32) × k ][ blsSig(256) ]`
- **committee(开,新)**:`[ nodeIdsLength(32) ][ ( nodeId(32) ‖ slot(32) ‖ merkleProof(TREE_DEPTH×32) ) × k ][ blsSig(256) ]`
  - `perSigner = 64 + TREE_DEPTH×32 = 64 + 14×32 = 512` 字节(`TREE_DEPTH=14`)。
  - `slot` = nodeId 在冻结委员会集 `setRoot[e-1]` 的槽位下标;`merkleProof` = 14 个 sibling hash 证明成员资格。
  - **`TREE_DEPTH()` 是 validator 上的 public 常量;SDK 应链上读、不硬编码**(dvt 确认)。

## 3. ⚠️ 命门 B2 — accountId 绝不进签名

- 委员会 payload 第 1 个 32 字节是 accountId,**由账户在 `validate()` 前注入 `address(this)`**。
- **SDK/KMS 产出的签名里绝不能含 accountId** —— 提交方提供 accountId = 直接破 2/3(seed 揭示后 shop accountId 塞自己节点)。
- accountId 只出现在:①链上委员会抽签 `H("CMT_SELECT", epochSeed, accountId, nodeId) < T` ②账户注入的 framing。**不进 BLS 签名消息。**

## 4. BLS 签名原像(KMS 最关键的一条,已双方逐字节确认)

**节点(含 co-located KMS-TEE)签的 BLS message = `bytes(userOpHash)` 32 字节,什么都不 prepend。committee 不改这个。**

- 账户调 `validator.validate(userOpHash, payload)`;验证器 #237 内部 `messagePoint = _hashToG2(userOpHash)`(RFC 9380 hash-to-curve),BLS 聚合就验这个点。
- **域分离全靠 DST**:`BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_`(含 `_POP_` 后缀)。链上 validate 用**逐字节相同**的 DST 重算,golden vector 锁 blst(KMS/dvt)↔noble(SDK)↔validator 三方一致。
- **KMS 侧核实**:`kms/ta/src/bls.rs:8` `BLS_DST = "BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_"`;`bls_sign(BlsSignInput.message: [u8;32])` 内部 `sk.sign(message, BLS_DST, &[])` = `hash_to_curve(message, BLS_DST)` + G2 sign。**与 dvt 描述逐字节一致 → KMS-TEE BLS 零改动。**

## 5. 模式判定 + quorum(KMS 不读,SDK 读;记录以备)

- `committeeActive()` = validator `epochLength != 0`,从 `router.getAlgorithm(0x01)` 读。关时账户字节级不变(向后兼容)。
- `requiredQuorum()` = `⌈2·m_e/3⌉`,`m_e(N) = clamp(⌈N/5⌉, 30, 86)`(N>8)。**下限是 30 不是 17**(dvt 纠正:17 是 DSR 早期提案,已被双尾 ε≤1e-6 取代)。
- **committee 在 N=3 即可激活**(只看 `epochLength` 翻转;N=3 退化 whole-set 2-of-3)。首轮 E2E 在 N=3 跑机制验证 —— **不阻塞在节点数**,阻塞在 SDK per-signer wire + dvt 翻 `setEpochLength`(CC-104)。

## 6. 影响的签名类型(BLS 块出现处) — 供 SDK,KMS 不组装

triple(0x01) · cumulative T2(0x09) · T3(0x0a) · T2WA · T3WA · weighted(0x07 bit-2 块)。每个 tier 签名里的 BLS 块按第 2 节替换;tier 其余部分(P256/owner ECDSA/guardian/WebAuthn/bitmap)不变。

## 7. Sepolia 集成地址(已验证)

| 组件 | 地址 |
|---|---|
| Factory(建账户) | `0x25C1E9F9120a406581f93bA82f7Cfd6805512791` |
| Router(getAlgorithm) | `0xA15127e8601e77De7C655bf04ca75cccD8C968f0` |
| `0x01` = committee validator | `0x1A8Db639b5d8Bd5742edB083656EDD56f416cd64`(3 节点,committeeActive=false) |
| Impl | `0x4873b7C1c07BE1b52d6583A64F5E902e593BDdad` |
| 已 enroll 测试账户 | `0xf249d5708cC3e1Dff42F5B36935FF270BeC403A0` |

- 现 `committeeActive=false` → 先跑 legacy 集成(签名原像同上,零变化);committee 正向等 dvt 翻 setEpochLength + SDK per-signer 就绪。
- **#161**:`InitConfig` 必带 `tier1Limit`/`tier2Limit`(0=unset 也要有字段,否则 `createAccount` 抛 "undefined to BigInt")。KMS 若不建账户则不受影响,建账户流程需注意。

## 8. KMS 角色边界(定稿,不可破)

- **KMS = 不透明签名机**:owner secp256k1(`sign_hash`)/ keeper secp256k1 / session P256 JWT / BLS partial `bls_sign([u8;32]=userOpHash)`(CC-24/34 KMS-TEE BLS 托管)。
- **不组装 tier 签名、不构造 `nodeId‖slot‖merkleProof` wire、不读 `committeeActive/requiredQuorum/TREE_DEPTH`、任何路径不产出/注入 accountId。**
- **不在 TEE 加 accountId domain 绑定**(dvt 纠正:方向反了,会破坏不透明边界 + 引 TEE↔链上漂移)。绑定靠 userOpHash(绑 sender/nonce)+ 链上 sortition + owner-gate 三层。
- owner(0x02)/session(0x08)/BLS partial 三者对 CC-98 **全零改动**。

## 9. 跨仓后续(非 KMS,记录以免误判"全关闭")

- **SDK**:per-signer wire 组装(committee 模式)+ 从 validator 链上动态读 `TREE_DEPTH()` + enroll 流程暴露 `enrollInCommitteeValidator()`。
- **dvt**:翻 `setEpochLength` 激活 committee(CC-104 翻转/迁移联锁)。
- **airaccount-contract**:账户侧硬编码 perSigner=512 的 liveness 耦合(dvt 改 depth 需协调改版)。
