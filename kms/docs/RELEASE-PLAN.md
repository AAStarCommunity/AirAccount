# AirAccount KMS 发布计划梳理

> 日期：2026-06-12 | 当前：Beta1 (v0.19.0 生产中)
> `fix/review-bugfix` 分支 = Beta2 候选（已整合 + 真机 E2E 通过）

## 当前状态

- **Beta1 / v0.19.0**：生产中，基础功能 + MX93 真机部署
- **`fix/review-bugfix` 分支**：已整合 PR #43/#44/#45/#46/#47 全部代码 + 审计 P0/High 修复 + 真机修复（M-4 TLS 污染、REE-FS fallback、4096 签名、build 流程）。2026-06-12 真机全链 E2E 通过（CreateKey/derive/SignHash/Sign/DeleteKey/WebAuthn）。

---

## Beta2（下一个发布）— 核心安全 + 稳定性

**全部已在 `fix/review-bugfix`，真机验证过：**

| 内容 | 来源 | 状态 |
|------|------|------|
| RPMB 钱包存储（+ REE-FS fallback） | PR #47 + 真机修复 | ✅ 已验证 |
| RPMB 反回滚计数器 | PR #43 | ✅ |
| TA 侧 WebAuthn rpId + UP 验证 | PR #44 | ✅ |
| dirf.db 0 字节自动修复 | PR #45 | ✅ |
| 自动备份系统 | PR #46 | ✅ |
| 审计 P0/High 修复（命令ID/超时/passkey强制/submodule） | 2787402 | ✅ |
| M-4 TLS 污染修复（真机抓的 panic） | c6aff3f | ✅ |
| MX93 build 流程 + 文档 | mx93-build.sh + BUILD-MX93.md | ✅ |
| MX93 部署基线（TRNG/超时/部署文档） | PR #35 | 待合并 |
| Apache 2.0 license / CLA | PR #33 / #2 | 待合并 |

### 评估后**提前到 Beta2**（避免发布后返工）

| 内容 | 来源 | 为什么提前 |
|------|------|-----------|
| **#41 ForceRemoveWallet**（孤儿清理） | **已在 #35**（commit b74b5f1, `ForceRemoveWallet=23`） | 随 #35 自动进 Beta2 ✓ 无额外成本 |
| **#6 P2 便利签名器**（SignMicropaymentVoucher / X402 / Authorization） | `feat/p2-sp-signers`（已实现，待 rebase + 测试） | SuperPaymaster v5.3.3 已 landed；不带则 app 集成新 SDK 缺端点 → 被迫发 Beta2.x。已实现只需 rebase，低成本 |
| **#21 EIP-712 domain = aastar.io** | 随 P2 signers 顺便对齐 | 域名一步到位，避免后续再改 |

注：#6 **P1 `SignTypedData` 已在 main**，Beta2 自带通用 EIP-712 能力（可签任意 typed-data，包括 voucher/x402/GToken，caller 自拼）。P2 只是便利封装（KMS 内部构造），是 DX 优化非功能缺失，但提前可省一次发布。

**待开发/测试**（Beta2 发布前完成）：rebase `feat/p2-sp-signers` → fix/review-bugfix（注意命令 ID 不与现有冲突），补 P2 端点的真机 E2E。

**PR 处置**：#43/#44/#45/#46/#47 关闭（内容已在 fix/review-bugfix），由 fix/review-bugfix 开**一个统一 PR** 取代；#35 合并；#33/#2 合并；P2 signers rebase 进同分支。

---

## Beta3 — 安全加固 + 生态对齐

| Issue | 内容 | 优先级 |
|-------|------|--------|
| **#49** | WebAuthn challenge binding（H-2 重放攻击）— 协议改动大（GetChallenge+nonce），若 Beta2 来得及则提前，否则 Beta3，**主网前必须** | **High** |
| **#6 P3** | SuperPaymaster UserOp v0.7 paymaster Sepolia E2E（需外部测试环境，不阻塞核心） | P1 生态 |
| #15 | TA 侧 JWT exp 检查（相对 TTL + TEE 时间） | P1 |
| #42 | 密钥生命周期管理 Phase 1（last_used_at + 状态字段） | P2 |

---

## 主网（Mainnet）发布前**必须**闭环 — 安全关键

| Issue | 内容 | 为什么主网前必须 |
|-------|------|-----------------|
| **#49** | WebAuthn challenge binding | **重放攻击**：被攻陷的 CA 可重放单条 assertion 无限签名。主网真金白银前必修 |
| **#50** | RPMB 生产编程（或正式评估 REE-FS 可接受） | secp256k1 私钥的硬件防回滚。需硬件定版后一次性编程（不可逆） |
| **#37** | TEE 远程证明（Attestation） | 让客户端密码学验证签名来自真实 OP-TEE，强烈建议主网前做 |
| — | 全部审计 P0/High 闭环 | 已在 Beta2 完成，主网前复查 |

---

## 当前未解决问题（主网前必须解决）

1. **#49 WebAuthn 重放攻击（最高优先）**：需新增 `GetChallenge` TA 命令 + nonce 机制 + CA 透传完整 clientDataJSON，TA 内比对 challenge。是 #39 信任链的最后一块。
2. **#50 RPMB 生产编程**：当前 REE-FS fallback 让基础工作（不阻塞 Beta2），但主网要硬件防回滚需编程 RPMB key（不可逆，硬件定版后做）。
3. **secp256k1 硬件**：已调研定论 —— i.MX ELE 不支持，软件 k256（~60ms）够用；#40 缩小为 P-256/SHA 加速；硬件 secp256k1 需外接 SE051（可选，非必须）。详见 `secp256k1-hardware-analysis.md`。

---

## 可选 / Post-Mainnet（不阻塞主网）

| Issue | 处置 |
|-------|------|
| #40 | 缩小为"P-256 passkey + SHA 的 ELE 加速"；secp256k1 软件够用 |
| #48 | ELE 当信任根（HUK/TRNG/P-256/attestation），非私钥存储 |
| #38 | PKCS#11 接口 — 需先定"哪类 key 可绕过 WebAuthn"安全红线 |
| #36 | **可关闭** — 已被 #43+#47（RPMB 反回滚 + 存储）覆盖 |
| #11 | 合约侧通知，跑字节级向量验证后关闭 |

---

## 开放 PR 一句话建议

| PR | 建议 |
|----|------|
| #47 RPMB 存储 | 关闭，并入 fix/review-bugfix 统一 PR → **Beta2** |
| #46 备份系统 | 同上 → Beta2（注意：单独合并会砖存量钱包，必须走统一 PR） |
| #45 dirf 修复 | 同上 → Beta2 |
| #44 WebAuthn TA | 同上 → Beta2 |
| #43 反回滚 | 同上 → Beta2 |
| #35 MX93 部署 | **直接合并 → Beta2**（生产基线，已在跑） |
| #33 README license | 合并 → Beta2（或随时） |
| #2 Apache 合规 | 合并 → Beta2（CLA + CONTRIBUTING） |
