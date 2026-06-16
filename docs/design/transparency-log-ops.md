<!-- Created: 2026-06-16 -->
# 透明日志 (B) —— 信任模型 + 运维方案

> 创建：2026-06-16 +07（本机时间）
> 关联：[`measurement-provenance-design.md`](./measurement-provenance-design.md) · [`attestation-trust-root-decision.md`](./attestation-trust-root-decision.md) · 验证器 `packages/attestation-verifier` · issue #87
> 状态：验证侧 + 发布侧已实现并对公共日志端到端验通（PR #89/#91/#92）。本文档定信任语义 + 上线运维。

---

## 1. 从信任角度，这套公共日志解决了什么？

### 原来的缺口：单一发布者私钥
attestation measurement manifest 由 **AAStar 一把 Ed25519 私钥**签名。客户端 pin 这把公钥验签。问题：**这把私钥若泄露或被胁迫**，攻击者可以签一份"恶意 manifest"（把一个后门 TA 的 measurement 列为 `current`），**单独发给某个受害者**，受害者无法分辨——因为签名是有效的。

### 透明日志补的：可检测性 + 不可抵赖（不是"阻止"，是"问责"）
要求**发布者签过的每一份 manifest 都进公开 append-only 日志**（见证人共签）：

- **可检测性**：恶意 manifest **没法偷偷发**。要么它进了公开日志（任何人——社区、监控、我们自己——都看得到这次"发布"）；要么它没进日志、过不了客户端的 Tier-2 校验。攻击者无法对单一受害者做"定向隐秘投毒"。
- **不可抵赖 / 防分叉**：见证人共签（quorum）保证日志**不能对不同人显示不同内容**。
- **信任转移**：从"相信 AAStar 的 key 永不被滥用" → **"任何滥用都会公开可见、且会被发现"**。这跟 Certificate Transparency 是同一个范式——**问责制，不是预防**。

> ⚠️ 它**不**证明 measurement 对应的代码无恶意（那是**可复现构建**：任何人从开源源码重算 measurement 比对）。也**不**锚定 NXP（那是 R-1）。它给的是**问责**：AAStar 不能偷偷改自己声称在跑的 TA。三件事叠起来才是完整信任：**可复现构建（代码可验）⊕ 透明日志（发布可审计）⊕ DVT（独立共签）**。

### 外部用户如何确认我们可信（验证流）
```
1. GET /attestation?nonce=<rand>      → evidence（含 ta_measurement）
2. GET /.well-known/attestation-measurements.json  → 签名 manifest + Sigsum proof
3. verifyMeasurementManifest(manifest, 已pin的发布者公钥, {transparency:{proof, 见证人policy}})
     ├─ 验发布者签名（pin 的 key）
     ├─ 验 manifest 在公开日志里、且 ≥quorum 见证人共签（Tier-2）
     ├─ 绑定：proof 记录的 == SHA256(这份 manifest body)  ← 防把别的 proof 套过来
     └─ measurement 状态 current/未 revoked
4. verifyAttestation(evidence)        → 运行的 TA measurement ∈ 已核准集
```
**用户得到的保证**：正在跑的 TA 的 measurement，是一个**被公开承诺过**的值。AAStar 若曾发布过任何不同/恶意的 measurement，它就躺在公开日志里，任何人都查得到。配合可复现构建，用户还能自己把这个 measurement 重算回开源源码。

---

## 2. 运维方案

### 心智模型：发布时动作，不是运行时动作 ✅ 关键
**manifest 只在 measurement 变化时才变**（= 新 TA 构建 / 吊销某 measurement），也就是**发版时**。所以：

- ❌ **不需要**每次启动 submit、**不需要**每个请求 submit、**不需要**常驻 daemon、**不需要**板子上加进程或开机 hook。
- ✅ KMS host 运行时**只是静态地服务 (manifest + proof) 文件**，**运行时根本不连日志**。
- ✅ 与日志的交互**只发生在发版流水线里**（一次性 + 每次发版）。

> 你担心的"要不要加启动/监控勾子、常驻进程"——**运行时不用**。只有发版流程 + 一个定时监控（见 §2.4）。

### 2.1 一次性设置
1. **生成 Sigsum 提交者密钥**（`sigsum-key gen -o submit`）。**私钥存成 CI secret / 离线保管**；公钥 hex pin 进文档 + 验证器配置。
2. **policy 文件**（log + 见证人 + quorum）入库。**透明性用途，公共 `sigsum-test1-2025`（test.sigsum.org/barreleye + 3 见证人 quorum 2）够用**（已实测验通）。
3. **决定 proof 怎么随 manifest 走**：建议**单独 sidecar** `/.well-known/attestation-measurements-proof.json`（manifest 不变，proof 是其旁证）。

### 2.2 每次发版自动发布（CI，GitHub Action）
触发：release tag / 手动 `workflow_dispatch`。步骤（全自动）：
```
1. 可复现构建 TA → 算出 ta_measurement（scripts/ta-measurement.sh）
2. 更新 manifest-body.json（version→measurement, status, sequence+1）
3. node scripts/sign-manifest.mjs   → 签名 manifest（用离线发布者 key 或 CI secret）
4. go install sigsum-go/cmd/sigsum-submit（CI 内装一次）
5. node scripts/submit-manifest-to-sigsum.mjs \
     --manifest <signed> --submit-key $SUBMIT_KEY --policy policy \
   → 提交日志、收齐 quorum 见证人共签、产出 proof sidecar JSON
6. 把 (manifest + proof sidecar) 发布到 host 服务目录（commit 进 kms/host/ 或部署上传）
```
所需 secret：发布者 Ed25519 私钥（签 manifest）+ Sigsum 提交者私钥。其余全公开。

### 2.3 host 改动（小）
现在 host 用 `include_str!` 编译进 manifest 并服务 `/.well-known/attestation-measurements.json`。需补：**同样方式 include + 服务 proof sidecar** `/.well-known/attestation-measurements-proof.json`。**唯一的 host 代码改动**，仍是静态服务、运行时不连日志。

### 2.4 B-4 监控（定时，不是常驻）
**目的**：日志让滥用"可见"，监控让滥用"被发现"。一个**定时 GitHub Action（cron，如每 6h）**，**纯 CI、无常驻进程**：

- **主监控（推荐，简单且打到真实攻击面）**：拉 `https://kms.aastar.io/.well-known/`（manifest + proof），跑 `verifyMeasurementManifest`（Tier-2 全验）+ 交叉核对 measurement ∈ **本仓库 git-tag 发布过的集合**（将来加 C 链上注册表则也核对链上）。**任一不符 → 告警**（issue / Slack）。这抓的是：被换的 manifest、没进日志的 manifest、不在我们发布集里的 measurement——即用户真正会拉到的东西。
- **深度监控（可选）**：扫 Sigsum 日志新叶子里**本提交者 keyHash** 名下的条目，比对是否都对应我们正式发布的 manifest → 检测**提交者私钥被盗后偷偷记日志**。更彻底但更重。
- 失败处理：告警 + 人工介入（吊销/轮换提交者 key、撤下被换 manifest、发新版）。

### 2.5 "需要我们做的额外工作"清单
| 项 | 一次性 / 每次 / 运行时 | 工作量 |
|---|---|---|
| 生成提交者 key + policy 入库 | 一次性 | 小 |
| host 服务 proof sidecar | 一次性（小代码） | 小 |
| 发版 CI publish 步骤 | 一次性搭，之后每次发版自动 | 中 |
| B-4 定时监控 Action | 一次性搭，之后自动 | 小-中 |
| **运行时（板子/KMS 进程）** | **无** —— 不加进程、不加 hook、不连日志 | **零** |

---

## 3. 小结
- **信任收益**：把"信 AAStar 一把 key"换成"AAStar 改不了已公开承诺的 TA measurement，且任何滥用公开可查"——问责制，CT 同源。
- **运维**：**发版时**自动 submit（CI）+ **定时**监控（CI），**运行时零额外负担、无常驻进程**。
- 透明性用途，**继续用公共 `sigsum-test1-2025` 即可**，不必自建日志。
