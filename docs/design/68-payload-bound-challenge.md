# #68 — Payload-Bound Challenge（闭合 V4：CA 偷换 payload）

> 创建时间：2026-06-14 16:10 +07（本机时间）
> 关联：威胁模型 `threat-model-ca-adversary.md` 向量 V4 · 安全路线图 A 线 · #49（challenge binding 基础）/ #63（strict）

## 问题（V4）

#49/#63 的 challenge binding 证明「用户在场 + 批准了一次带 challenge `C` 的 WebAuthn ceremony」，但 `C` 只是绑 `wallet_id` 的随机 nonce，**不绑「签的是什么」**。一个被攻陷的 CA 可以：拿到用户对 `C`（本应签 `D_legit`）的合法 assertion，转手提交 `SignHash(wallet, D_evil, assertion(C))` —— TA 只校验 challenge 匹配，**不校验 payload**，于是签了用户没批准的 `D_evil`。

## 方案：把 challenge 绑定到 payload 摘要

1. 客户端调 `GetChallenge` 时附带**本次要签的摘要 `D`**（如 userOpHash / SignHash 的 hash / typed-data hash）。
2. TA 签发 nonce `C`，在 pending 表里存 `(wallet_id, C, D, issued_at)`。
3. 签名时，TA 取**实际待签的摘要 `H`**，校验：`assertion.challenge == C` **且** `H == D`（绑定的摘要）。不符即拒。

**为什么闭合 V4**：恶意 CA 即便调 `GetChallenge(wallet, D_evil)` 拿到为 `D_evil` 绑定的 `C'`，也**拿不到用户对 `C'` 的 WebAuthn 授权**（用户只在可信客户端里对 `D_legit` 的 `C` 授权过）。CA 想用 `D_legit` 的 assertion(`C`) 去签 `D_evil` → TA 查 `C` 绑的是 `D_legit ≠ D_evil` → 拒。assertion 从此**只能签它被授权的那一个 payload**。

信任假设：`GetChallenge` 的 `D` 来自可信客户端（用户的 app/SDK），用户的 WebAuthn 批准对应该 ceremony。CA 不可信但绕不过「用户没给 `C'` 签名」这一点。

## 实现计划

**proto（`kms/proto`）**
- `GetChallengeInput` 增 `payload_digest: Option<[u8;32]>`（`#[serde(default)]` 保 bincode 兼容）。strict 下要求 `Some`。
- `PendingChallenge`（TA 内部结构）增 `payload_digest: Option<[u8;32]>`。

**TA（`kms/ta/src/main.rs`）**
- `challenge_issue(wallet_id, payload_digest)` 存绑定。
- `challenge_peek/consume` 返回 `(nonce, issued_at, payload_digest)`。
- `verify_challenge_binding(wallet_id, assertion, actual_payload_digest)`：在现有 nonce 校验后，若绑定表有 `payload_digest`，则要求 `actual_payload_digest == 绑定值`；strict 下要求绑定必须存在。
- `verify_passkey_for_wallet` 增 `payload_digest: Option<&[u8;32]>` 参数，透传给 `verify_challenge_binding`。
- 各签名 handler 计算自己的待签摘要并传入：
  - `sign_hash` → `input.hash`
  - `sign_transaction` → 待签 txn 的哈希
  - `sign_message` → 消息哈希
  - `sign_typed_data` → EIP-712 digest
  - `sign_grant_session` / `sign_p256_grant_session` → grant 摘要
  - agent / p256 userOp 路径 → userOpHash
  - 非签名敏感操作（如 derive/register/remove）可传 `None`（无 payload 概念；strict 仍要求 challenge 但不绑 payload，或单列策略）

**host（`kms/host`）**
- `TeeHandle::get_challenge(wallet_id, payload_digest)` + handler/路由把 payload digest 传进去。

**SDK（#58 / A3）**
- 升级流程：先算 payload 摘要 → `GetChallenge(wallet, digest)` → 用返回 nonce 做 WebAuthn challenge → 签名请求带 assertion。

**测试（真机 E2E）**
- 正例：`GetChallenge(W, D)` → 对 D 的签名通过。
- 反例（V4）：`GetChallenge(W, D_legit)` 拿 assertion，提交签 `D_evil` → **拒**（payload 绑定不符）。
- 反例：无 payload_digest（strict）→ 拒。

## 兼容/顺序

- 与 #63 strict 协同：strict 镜像下，缺 `clientDataJSON` 或缺 payload 绑定都拒。
- 依赖 SDK #58 把新流程（GetChallenge 带 digest）发出来。生产无老客户（用户确认），可直接上。
- bincode 跨版本：host+TA 同版部署（一贯如此）。
