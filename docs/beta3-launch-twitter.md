# Beta3 (v0.21.0) 发布 — Twitter / 社媒文案

> 2026-06-13 · 复制即用。English thread + 中文版。

---

## English (thread)

**1/4**
🚀 AirAccount KMS Beta3 (v0.21.0) is live.

The identity & key layer of the Mycelium Protocol ecosystem — TEE key management + WebAuthn + AWS KMS-compatible API. Theme this round: hardening + ecosystem alignment. 🔐

**2/4**
The headline: WebAuthn challenge binding now lives INSIDE the TEE.

The TA mints a one-time nonce (hardware TRNG), binds it to the wallet, and burns it on use. Even a fully compromised server can't replay a captured signing authorization. (closes H-2)

**3/4**
Decentralized by construction:

🧊 No admin in release builds — /admin/purge-key is compiled out entirely (not an env switch). Zero admin symbols in the binary.
⏳ Dormant-key freeze: idle keys auto-freeze; only the owner's WebAuthn ceremony unfreezes.
🔗 EIP-3009 `from` bound to your real derived address — no wasted-gas reverts.

**4/4**
Verified the hard way: 41/41 real-device E2E on NXP i.MX93, plus a 4-round adversarial PK review (DeepSeek / Sonnet / Codex / Opus) — all APPROVED.

Next → mainnet: RPMB programming, remote attestation.

Apache 2.0 · open source
👉 github.com/AAStarCommunity/AirAccount

---

## 中文（thread）

**1/4**
🚀 AirAccount KMS Beta3 (v0.21.0) 发布。

Mycelium Protocol 生态的身份与密钥底层 —— TEE 私钥管理 + WebAuthn + AWS KMS 兼容 API。本轮主题:安全加固 + 生态对齐。🔐

**2/4**
最大亮点:WebAuthn challenge binding 下沉进 TEE。

TA 用硬件 TRNG 生成一次性 nonce,绑定钱包、用后即焚。即使运行服务器被完全攻陷,也无法重放一条捕获的签名授权。(关闭 H-2 重放)

**3/4**
去中心化是编译进去的:

🧊 正式版没有 admin —— /admin/purge-key 被整个编译掉(不是 env 开关),二进制里零 admin symbol。
⏳ 久置密钥保护:长期不用自动冻结,只有 owner 的 WebAuthn ceremony 能解冻。
🔗 EIP-3009 `from` 绑定到你 key 派生的真实地址,杜绝白烧 gas 的链上 revert。

**4/4**
硬碰硬验证:NXP i.MX93 真机端到端 41/41,外加 4 轮对抗式 PK review(DeepSeek / Sonnet / Codex / Opus)—— 全部 APPROVED。

下一站 → 主网:RPMB 编程、远程证明。

Apache 2.0 · 开源
👉 github.com/AAStarCommunity/AirAccount

---

## 单条精简版（如果只发一条）

🚀 AirAccount KMS Beta3 (v0.21.0):WebAuthn 防重放下沉进 TEE(一次性 nonce 用后即焚)、正式版零 admin(编译掉)、久置密钥冻结、EIP-3009 from 绑定。NXP i.MX93 真机 41/41 + 4 轮对抗 review 全过。私钥永不出 TEE。开源 Apache 2.0 👉 github.com/AAStarCommunity/AirAccount
