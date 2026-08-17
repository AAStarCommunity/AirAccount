# KMS 签名边界 — CC-98 committee 与 accountId 命门(B2)

> 来源:CC-103(Seeder b8f3441f)定稿规范。权威细节见 `docs/agent/cc103-kms-committee/SPEC.md`。
> 本文是 KMS 侧对"committee 变更为何对 KMS 零改动 + accountId 绝不进签名"的**不可破边界**声明。

## 1. KMS = 不透明签名机(不可破边界)

KMS 对给定 digest/message 出签名,**不组装 tier 签名、不构造委员会 wire、不注入 accountId**:

| 签名类型 | 输入 | 输出 | committee 影响 |
|---|---|---|---|
| owner ECDSA (0x01/0x02) | 32B digest / userOpHash | secp256k1 65B / WebAuthn | **零** |
| keeper ECDSA | 32B digest | secp256k1 65B | **零** |
| session (0x08) | — | `[0x08][account(20)][keyX][keyY][r][s]` 149B | **零** |
| **BLS partial** `bls_sign` | **`[u8;32]` = userOpHash** | EIP-2537 G2 256B | **零** |

## 2. BLS 原像不变(committee 的核心澄清)

- 节点(含 co-located KMS-TEE)签的 BLS message = **`bytes(userOpHash)` 32B**,什么都不 prepend。
- 链上验证器 `messagePoint = _hashToG2(userOpHash)`(RFC 9380);域分离全靠 **DST**:
  `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_`(`kms/ta/src/bls.rs:8`,与 dvt #237 validator + SDK noble 三方 golden 一致)。
- **committee 不改这个原像。** committee 新增的 `slot`/`merkleProof`(成员证明)与 `accountId`(抽签输入 + 账户注入 framing)**都不进 BLS 签名消息**。
- ⇒ **KMS-TEE `bls_sign` 今天怎么签,committee 开了还怎么签,零改动。**

## 3. ⚠️ accountId 命门(B2)—— 两种"account"必须分清

**B2 的 accountId**:委员会 BLS payload 的 **32 字节** accountId,决定谁在委员会(`H("CMT_SELECT", epochSeed, accountId, nodeId) < T`)。**由账户在 `validate()` 前注入 `address(this)`,绝不能进 KMS/SDK 产出的签名** —— 提交方提供 accountId = 破 2/3(seed 揭示后 shop accountId 塞自己节点)。

**KMS 侧审计结论(2026-08-18,feat/cc103-kms-committee-audit)**:
- **A1**:`kms/ta/src/`、`proto/in_out.rs`、`ta_client.rs` 中 accountId 构造/注入 = **0 处**。BLS/owner/keeper 路径均对 digest 盲签,不含 account。
- **A2**:签名 I/O 结构中唯一的 "account" 字段 = **会话签名(0x08)wire 里的 20 字节 ERC-4337 账户地址**(`proto/in_out.rs:462/491`)。**这不是 B2 的 accountId**:
  - 它是 **20 字节**(B2 是 32 字节),是**会话密钥的账户绑定**(注释:"prevent cross-account abuse"),防会话签名跨账户重放;
  - 它在 **session 0x08**,与 committee BLS 块**无关**;session 0x08 按规范本就**不受 committee 影响**;
  - BLS partial(`bls_sign`)**不含任何 account**,只签 userOpHash。
  - **∴ 不违反 B2。** 两者是正交机制,勿混淆。

## 4. 明确不做(方向性红线)

- **不在 TEE 内加 committee 的 accountId domain 绑定**(dvt 已纠正:方向反了,会破坏本"不透明签名机"边界 + 引入 TEE↔链上语义漂移)。绑定靠 userOpHash(绑 sender/nonce)+ 链上 sortition + 节点 owner-gate 三层。
- **不改 `bls.rs` 的签名/hash-to-curve 逻辑**(原像未变)。若未来 dvt 改 #237 的 DST,才需改 `BLS_DST` + 重编 TA + 刷板 + golden 回归 —— 由 golden-vector 回归测试(见 `docs/agent/cc103-kms-committee/ACCEPTANCE.md §B`)守护。

## 5. 跨仓(非 KMS,记录防误判)

committee 正向路径的 per-signer wire 组装(SDK)、链上读 `TREE_DEPTH()`(SDK)、翻 `setEpochLength`(dvt/CC-104)、账户侧 perSigner=512 耦合(airaccount-contract)——均非 KMS。KMS 侧 committee 天然就绪。
