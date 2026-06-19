# AirAccount KMS 发布计划梳理

> 日期：2026-06-19 | 当前：**Beta5 (v0.23.1) 已发布** ✅

## 当前状态

- **Beta2 / v0.20.0**：**已发布**（2026-06-12）。整合 PR #51/#35/#33/#2 全部合并进 main，真机 FRDM-IMX93 全链 E2E **34/34** 通过。详见 [CHANGELOG 0.20.0](../CHANGELOG.md)。
- **Beta1 / v0.19.0**：前一版本，基础功能 + MX93 真机首次部署。
- **Beta2 内容**：审计 P0/High 全修 · RPMB 存储+反回滚 · TA 侧 WebAuthn 验证 · MX93 部署(gap-key/CAAM-bypass/ForceRemoveWallet) · P2 SuperPaymaster 便利签名器(ceremony 鉴权) · agent-key TA panic 根治 · 测试私钥移出 git · Apache 2.0 合规。

## 测试覆盖（2026-06-12 真机验证）

| 层 | 命令 | 结果 |
|----|------|------|
| **E2E（真机 MX93，100% 端点）** | `kms/test/run-full-e2e.sh`（上板跑） | **34/34 通过**（30 核心 + 3 P2 便利签名器 + 1 负向） |
| 单元测试 · proto | `cargo test --manifest-path kms/proto/Cargo.toml` | **39 通过** |
| 单元测试 · host（CA） | `kms/test/run-host-unit-tests.sh`（交叉编译 → 上板跑） | **56 通过** |
| 单元测试 · TA | 不适用 | 见下 |

- **E2E 30/30** 用真实 WebAuthn ceremony（无 legacy passkey 捷径），覆盖每个功能端点：注册（Begin/Complete/BeginAuth + begin-grant-session-auth）、密钥生命周期、派生/签名、ChangePasskey、agent key（create/sign/refresh/revoke）、EIP-712 SignTypedData、grant session（secp256k1 + p256，purpose 绑定 challenge）、p256 session（create/sign-user-op/revoke）、负向 auth-gate 拒绝。
- **host 单元测试**因链接 optee-teec（需 libteec，容器仅 ARM 库）必须交叉编译成 aarch64 上板跑 —— `run-host-unit-tests.sh` 封装了这一流程。
- **TA 单元测试**：`aarch64-unknown-optee` target 无 std test harness，`cargo/xargo test` 不可行。TA 命令逻辑由真机 E2E 全路径覆盖；grant-session ABI 编码另经 codex 逐字节对照合约审计。未来如需隔离 KAT，可把纯逻辑（eip191_hash / build_grant_session_inner / agent_derivation_path）抽到 no_std 共享 crate 在 host 测。

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
| **#6 P2 便利签名器**（SignMicropaymentVoucher / X402 / Authorization） | `feat/p2-sp-signers` → **已集成进 fix/review-bugfix（commit f3244d5），真机 33/33 通过** ✅ | SuperPaymaster v5.3.3 已 landed；不带则 app 集成新 SDK 缺端点 → 被迫发 Beta2.x |
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

## 编译期 feature 门控（去中心化定位 — 正式版无 admin）

AirAccount 是**去中心化 KMS**，正式发布版**不能含任何 admin/超级用户面**。危险/运维工具一律走**编译期 feature**（默认不编译进二进制，物理上不存在），而非运行时 env gate（env 可被人重新设上而"复活"）。

| Feature | 门控内容 | 默认 | 何时启用 |
|---------|---------|------|---------|
| `export-secrets` | 助记词导出 + 无 passkey 的 `ExportPrivateKey` CLI（`export_key` bin） | OFF | 仅本地调试 |
| `admin-purge` | `POST /admin/purge-key` 端点（用 `KMS_ADMIN_TOKEN` 无 passkey 强删 key），测试期清理坏 key/孤儿/gap key 用 | OFF | 仅 beta/测试构建 |

**构建约定**：

- **正式 release（默认）**：`cargo build --release`
  → **不带任何 feature**，`/admin/purge-key` 端点的方法、handler、struct、route **全部不编译进二进制**，admin 面物理上不存在。即使有人设了 `KMS_ADMIN_TOKEN` 也无端点可调。
- **beta/测试构建**：`cargo build --release --features admin-purge`
  → admin 端点编译进二进制，仍需 `KMS_ADMIN_TOKEN` 运行时鉴权（双重门控）。

> 实现：route 定义在 `#[cfg(feature = "admin-purge")]` 块里折叠进 `group4`（re-boxed），使带/不带 feature 两条编译路径的 `routes` 链类型完全一致，无需在链上额外 `.or()`。两条路径都必须能编译通过（CI 应分别 build 默认 + `--features admin-purge` 验证）。

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

1. **#49 WebAuthn 重放攻击（最高优先）**：需新增 `GetChallenge` TA 命令 + nonce 机制 + CA 透传完整 clientDataJSON，TA 内比对 challenge。是 #39 信任链的最后一块。（注：P2 便利签名器已直接复用 `sign_typed_data` 的 ceremony 鉴权路径，与 SignTypedData 同等重放保护，无单独遗留项。）
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
