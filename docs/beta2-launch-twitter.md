# Beta2 (v0.20.0) 发布 — Twitter / 社媒文案

> 2026-06-12 · 复制即用。English thread + 中文版。

---

## English (thread)

**1/4**
🚀 AirAccount KMS Beta2 (v0.20.0) is live.

TEE private-key management + WebAuthn passwordless auth + AWS KMS-compatible API — the identity & key layer of the Mycelium Protocol ecosystem.

Private keys never leave the secure enclave. 🔐

**2/4**
What's new in Beta2:

🔒 Full security audit — all P0/High findings fixed
🔗 SuperPaymaster signers: gasless micropayment / EIP-3009 / x402 in one API call (no EIP-712 wrangling)
🛠️ Live on NXP i.MX93 + OP-TEE 4.8 hardware
✅ 100% E2E coverage: 34/34 on real device

**3/4**
Every signing op requires a live, replay-protected WebAuthn ceremony — a compromised server can't forge signatures.

secp256k1 wallet keys, software-guarded inside the TEE, with RPMB hardware anti-rollback.

**4/4**
Next stop → mainnet: WebAuthn challenge binding, RPMB production programming, TEE remote attestation.

Apache 2.0 · open source
👉 github.com/AAStarCommunity/AirAccount

---

## 中文（thread）

**1/4**
🚀 AirAccount KMS Beta2 (v0.20.0) 发布。

TEE 私钥管理 + WebAuthn 无密码认证 + AWS KMS 兼容 API —— Mycelium Protocol 生态的身份与密钥底层。

私钥永不离开安全飞地。🔐

**2/4**
Beta2 亮点：

🔒 完整安全审计，P0/High 全部修复
🔗 SuperPaymaster gasless 支付签名（微支付 / EIP-3009 无 gas 转账 / x402），一行 API 搞定，免拼 EIP-712
🛠️ NXP i.MX93 + OP-TEE 4.8 真机部署
✅ 真机端到端测试 100% 覆盖：34/34

**3/4**
每次签名都要一次实时、防重放的 WebAuthn ceremony —— 被攻陷的服务器也伪造不了签名。

以太坊 secp256k1 私钥在 TEE 内软件托管，配 RPMB 硬件反回滚。

**4/4**
下一站 → 主网：WebAuthn challenge binding、RPMB 生产编程、TEE 远程证明。

Apache 2.0 · 开源
👉 github.com/AAStarCommunity/AirAccount

---

## 单条精简版（如果只发一条）

🚀 AirAccount KMS Beta2 (v0.20.0)：TEE 私钥管理 + WebAuthn + AWS KMS API。安全审计全过、SuperPaymaster gasless 签名端点、NXP i.MX93 真机部署、100% E2E (34/34)。私钥永不出 TEE。开源 Apache 2.0 👉 github.com/AAStarCommunity/AirAccount
