# #68 — Payload-Bound Challenge（闭合 V4：CA 偷换 payload）

> 创建时间：2026-06-14 16:10 +07 · 重构：2026-06-14 19:30（commitment 方案，PR #75 review round-2）
> 关联：威胁模型 `threat-model-ca-adversary.md` 向量 V4 · 安全路线图 A 线 · #49（challenge binding 基础）/ #63（strict）

## 问题（V4）

#49/#63 的 challenge binding 证明「用户在场 + 批准了一次带 challenge `C` 的 WebAuthn ceremony」，但 `C` 只是绑 `wallet_id` 的随机 nonce，**不绑「签的是什么」**。被攻陷的 CA 可拿用户对 `C`（本应签 `D_legit`）的合法 assertion，转手 `SignHash(wallet, D_evil, assertion(C))` → 签了用户没批准的 `D_evil`。

## 为什么「把 D 存进 TA 表」不够（PR #75 review 抓到的两个变体）

第一版尝试：`GetChallenge(wallet, D)` 把 `D` 存进 TA pending 表，签名时校验实际 payload == 表里的 `D`。两个 V4 变体都绕得过：

- **strip 变体**：CA 剥掉 `PayloadDigest` → TA 发未绑定 nonce → 检查被跳过。（round-1 加 strict 强制门挡住了。）
- **substitute 变体（round-2 BLOCKING）**：用户的 authenticator **只签 clientDataJSON 里的 challenge `C`，`D` 根本不在用户签的内容里**。`C↔D` 绑定只存在 TA 表里、由 **CA 可控的明文 `PayloadDigest`** 设定。CA 直接把 `D_legit` 改成 `D_evil`（它就是 host）→ TA 绑 `C↔D_evil` → 用户对 `C` 签名（不知情）→ `SignHash(D_evil, assertion(C))` → 匹配 → 签了 `D_evil`。

**根因**：`D` 没有进入用户可证完整性的签名内容。任何「靠 CA 转发的明文字段建立的绑定」都不可信。

## 方案：commitment —— 把 D 塞进用户签的 challenge

让 `D` 成为用户 authenticator 实际签名内容的一部分：

```
客户端（可信）：  challenge = SHA-256(nonce ‖ D)      ← 用这个做 WebAuthn ceremony
TA（签名时）：    expected  = SHA-256(stored_nonce ‖ 实际待签payload)
                 require  assertion.clientDataJSON.challenge == expected
```

- `nonce`：TA `GetChallenge` 发的**纯随机值**（提供新鲜度 + 一次性，防重放）。
- `D`：本次要签的摘要（userOpHash 等）。
- 用户 authenticator 签的 clientDataJSON 含 `challenge = H(nonce‖D)` → **D 进了用户签名**。

**为什么同时关掉两个变体**：
- **substitute**：CA 把 `D_legit` 改成 `D_evil`，则 TA 重算 `H(nonce‖D_evil)` ≠ 用户签的 `H(nonce‖D_legit)` → 拒。CA 没有用户对新 commitment 的 assertion，造不出来。
- **strip**：CA 不让客户端 commit、改用 plain nonce 作 challenge → strict 下 TA 要求签名 op 的 challenge 必须等于 commitment（不接受 plain nonce）→ 拒。
- **GetChallenge 无 payload 字段**：没有 CA 可篡改的明文绑定字段，从源头消除 substitute 面。

## 实现（已落地）

**proto（`kms/proto`）**：`GetChallengeInput` **无 payload 字段**（注释说明为何）；`GetChallenge` 仍只返回随机 nonce。

**TA（`kms/ta/src/main.rs`）**：`verify_challenge_binding(wallet_id, assertion, expected_payload)`：
- `expected_payload = Some(D)`（签名 op）：算 `committed = SHA256(nonce‖D)`；
  - `challenge == committed` → 通过（用户签名 commit 到此 payload）；
  - `challenge == nonce`（plain）：strict **拒**（必须 commit）/ transition 放行（迁移态，不回归）；
  - 都不等 → 拒「does not commit to the payload」。
- `expected_payload = None`（非签名 op）：`challenge == nonce`（#49 行为）。
- 常量时间比较（`ct_eq32`，定长 32B）；在 consume 之前校验（不烧 victim nonce）。
- `verify_passkey_for_wallet` 增 `expected_payload`；**全部签名 op 都传各自待签摘要**：`sign_hash`→`input.hash`、`sign_transaction`→`Wallet::tx_signing_hash`(RLP keccak)、`sign_message`→`keccak256(message)`、`sign_typed_data`→`eip712_digest`(提前到 auth gate 前计算)、`sign_grant_session`/`sign_p256_grant_session`→`eip191_hash(inner)`。非签名 op（derive/register/remove/export/create-agent）传 `None`。

**host（`kms/host`）**：`BeginAuthentication` 无 `PayloadDigest` 字段；`get_challenge` 无 payload 参数。host 只返回 nonce，commitment 由客户端算。

**SDK（#58 / A3，aastar-sdk 仓库）**：签名流程 = 算 `D`(userOpHash) → `BeginAuthentication(wallet)` 拿 `nonce` → **challenge = SHA256(nonce‖D)** 做 WebAuthn → `SignHash(hash=D, assertion{…含 client_data_json})`。契约已在 aastar-sdk #58 更新。

## 剩余（非阻塞，follow-up）

- ~~5 个 sign op 未接线~~ **✅ 已接线**（feat/68-wire-remaining-signops）：`sign_transaction/message/typed_data/grant×2` 现各自计算待签摘要并传入 commitment 门 → **V4 对全部签名路径在 strict 下闭合**。SDK 须为每个 op 用对应的 D 计算 `challenge=SHA256(nonce‖D)`（D 定义见上 TA 行）。
- **正向 E2E**：需真 WebAuthn assertion（passkey 签 `H(nonce‖D)`）→ 靠 SDK（#58）。负例 E2E（CA 改 D → SignHash 拒）同样需 assertion。
- transition 模式 V4 仍开放（可接受，迁移态）；**mainnet 必须 strict 镜像 + commitment**。

## 兼容/顺序

- 与 #63 strict 协同：strict 镜像下，缺 `clientDataJSON`、或签名 op 的 challenge 不是 commitment（含改用 plain nonce）都拒。
- bincode 跨版本：host+TA 同版部署（一贯如此）。
