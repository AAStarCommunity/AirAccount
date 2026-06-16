<!-- Created: 2026-06-16 -->
# Attestation 信任根决策记录 —— NDA 受阻 + 安全评估 + 替代路径调研

> 创建：2026-06-16 +07（本机时间）
> 关联：[`37-remote-attestation-design.md`](./37-remote-attestation-design.md)（R-1）· [`security-roadmap.md`](./security-roadmap.md)（B 线）· [`threat-model-ca-adversary.md`](./threat-model-ca-adversary.md)（V5）· [`dvt-program-coordination.md`](./dvt-program-coordination.md)
> 目的：把「NXP NDA 申请受阻」这件事、它对安全/性能/目标的真实影响、以及**不依赖厂商信任根的远程证明替代路径**记录在案，避免反复纠结。

---

## 1. 背景：NXP NDA + 受限访问资格 申请受阻

为把 attestation 信任根锚定到 NXP（R-1），需要 NDA 受限文档（IMX93SRM 等）+ 账号级访问资格（EL2GO 账号、Secure Files entitlement）。

- **2026-06-15 提交 NXP.com NDA 申请**（Case **#00987060**，申请人个人/学生身份）。
- **NXP 回复（被拒走在线门户）**：个人（无注册法律实体）的 NDA 涉及更高审核与责任风险（违约责任等），**无法通过 NXP.com 受理**，需线下定制条款。NXP 明确「**含大学在内的法律实体均可签 NDA**」——拒的是**个人**，不是机构。
- 即便签了 NDA，还有第二道：**账号级 entitlement**（Secure Files 文档访问授权 + EL2GO 服务接入），对个人同样是卡点。
- 已草拟发 NXP 中国/代理商的求助邮件（走法律实体或代理商 sponsor 路径）。

## 2. 安全评估：受阻只影响「最高信任档」，不影响地板

**被卡的唯一东西 = R-1（把信任根锚定到 NXP，即"连部署方都不必信任"的全信任档）。** 其余全部已完成且不依赖 NXP：

| 维度 | 影响 |
|---|---|
| **安全** | 仅 V5 从「全信任」退到「**半信任**」（TOFU + 可复现构建 + 签名 manifest）。V1/V2/V3/V4 已闭、私钥不出 TEE、WebAuthn 强制，均不依赖 NXP。**且 V5 已有第二道独立缓解 = DVT 门限共签（#70），不需要 R-1。** |
| **性能** | **零影响**。NDA/attestation 是信任与验证层，与签名延迟/吞吐无关。（唯一沾性能的 #40 CAAM 加速早已因「ELE 不支持 secp256k1」作废，与 NDA 无关。）|
| **预期达标** | 不影响**地板**（产品对半去中心化定位安全可用、主网就绪、生态对接、生产部署均不卡 R-1），只影响**天花板**（能否宣称"对抗部署方本身"）。|

**双重不确定提醒**：R-1 即使签了 NDA 也未必能解 —— NDA 只让你能问"这条链存不存在"。若 i.MX93 ELE 没有可离线验证的 NXP 链，NDA 白费。故 **R-1 不应作为关键路径。**

### 依赖 NDA/账号的项（除 attestation 锚定外几乎没有）
| 项 | 依赖 NDA/账号？ |
|---|---|
| #37 Phase2 / R-1 / B 线 | ✅ 唯一真依赖 |
| #48 ELE/HSM 迁私钥 | ELE 不支持 secp256k1 → 本就基本作废，非损失 |
| #50 RPMB key 编程 / R-9 TA 侧 | ❌ 不需要（OP-TEE/eMMC 标准 provisioning）|
| D1 attestation 默认绑 / #63 strict flip | ❌ 不需要（等的是软件/SDK，非 NXP）|

---

## 3. 替代路径调研：不锚定厂商根，还能怎么做远程证明？

调研结论：远程证明的信任锚本质上只有**三大家族**，厂商 PKI 只是其一，且**对 Web3 场景，行业共识正在远离"信厂商"**。

### 信任锚三家族

| 家族 | 机制 | AirAccount 现状 |
|---|---|---|
| **(A) 厂商硬件 PKI** | NXP/Intel/AMD/ARM 出厂注入密钥 + 证书链（SGX/SEV/TDX quote、TPM EK、DICE、PSA）| ⛔ 卡 NDA；且只证"正宗芯片" |
| **(B) 可复现 + 透明** | 开源可复现构建 + 公开 append-only 透明日志 + 多见证人，任何人重算验证 | ✅ 已有可复现构建 + 签名 manifest；**可加透明日志** |
| **(C) 去中心化 / 经济信任** | 门限共签、链上 measurement 注册表、质押/罚没、共识验证者 —— 信任来自数量与激励而非根证书 | ✅ 已有 DVT 门限共签（#70）；**可加链上 measurement 注册** |

### 关键洞见（来自调研）
- **"Attestation 是信号，不是信任模型"**：attestation 只回答"什么代码、在哪、何时运行"，**不**回答"为何相信这个结果"（不含正确性、新鲜度、运营方问责、回滚历史）。真正的信任要叠加：**持续验证 + 版本化策略 + 共识/容错验证者网络**——而非把验证甩给单个客户端。这恰好印证 AirAccount 的 **DVT + 链上策略**方向。([dev.to](https://dev.to/caerlower/remote-attestation-is-a-signal-not-a-trust-model-2664))
- **去中心化信任根可行且已有产品**：Phala dstack 用 **MPC 管理根密钥（无单一厂商秘密）+ 链上智能合约治理（注册/黑名单/验 RA）+ 质押**，把信任从"你信不信 Intel"转成"你信不信这套去中心化共识"；Oasis 把 attestation 验证、运营治理、策略执行**嵌进协议共识**。([Phala](https://docs.phala.com/dstack/design-documents/decentralized-root-of-trust) · [Teamwork Makes TEE Work, arXiv:2402.08908](https://arxiv.org/abs/2402.08908))
- **供应链透明栈可移植到 attestation**：Sigstore/Rekor（公开透明日志 + keyless 签名）+ SLSA provenance —— 让 measurement 清单不再依赖单一发布者私钥，而是**公开可审计、防篡改**。([Sigstore](https://docs.sigstore.dev/logging/verify-release/))
- **标准化解耦信任根**：EAT（RFC 9711，CWT/JWT attestation token）+ Veraison verifier 消费**多来源 endorsement**（不限厂商），把"令牌格式"与"信任根"解耦。([RFC 9711](https://www.rfc-editor.org/info/rfc9711/))
- **更前沿**：TEE+TPM 多根协同（CCxTrust）、PUF 内生根 + 链上去中心化验证、**ZK 证明 attestation 有效性**（链上可验、不集中化）。([CCxTrust arXiv:2412.03842](https://arxiv.org/pdf/2412.03842) · [TikTok ZK attestation](https://developers.tiktok.com/blog/verifying-trusted-execution-environments))

### 对 AirAccount 的可落地选项（均不需要 NXP NDA）
1. **(B) 透明日志**：把签名 measurement manifest 发布到公开 append-only 日志（Rekor 风格或自建 + 多见证人co-sign）→ 去掉当前 manifest 的单一发布者信任点。
2. **(C) 链上 measurement 注册表**：把核准的 `ta_measurement` 上链（复用 SuperPaymaster 链上基础设施），客户端/SuperPaymaster 对链验证，配 DVT 质押/罚没 → 信任来自共识 + 经济。
3. **DVT 当 V5 主缓解**（已做）：与行业"共识而非厂商"共识一致；对"部署方是对手"，门限独立共签实务上比厂商 attestation 更硬。
4. **(D2) EAT/Veraison 输出**：标准化 token + 多 endorser verifier（路线图已列，靠后）。

---

## 4. 决策

1. **不把个人 NDA 当关键路径死磕** —— 影响天花板不影响地板，且可能签了也无解。
2. **要解锁 R-1 就走法律实体**（AAStar 组织 / 注册公司 / 大学 TTO）或代理商 sponsor —— 一步绕开 NXP 拒个人的理由；可并行慢推。
3. **信任根战略改为「(B) 可复现+透明 ⊕ (C) 去中心化/DVT」为主，(A) 厂商根为可选增强**。这既符合 AirAccount 半去中心化定位，也符合 Web3 attestation 的行业趋势。下一步具体增强：**透明日志（B）** 与 **链上 measurement 注册表（C）**，两者都不需要 NXP。
4. **窗口用在不卡 NXP 的安全项**：#50（RPMB 防回滚）、D1（attestation 默认化）、#63 strict flip。
5. **对外口径**：当前 TOFU + 可复现 + DVT 是**诚实、站得住的半去中心化信任模型**，已如实声明；不因未拿到 NXP 根而视为"未达标"。

---

## Sources
- [Remote Attestation Is a Signal, Not a Trust Model — dev.to](https://dev.to/caerlower/remote-attestation-is-a-signal-not-a-trust-model-2664)
- [Decentralized Root-of-Trust — Phala dstack](https://docs.phala.com/dstack/design-documents/decentralized-root-of-trust)
- [Teamwork Makes TEE Work: Open and Resilient Remote Attestation on Decentralized Trust — arXiv:2402.08908](https://arxiv.org/abs/2402.08908)
- [Sigstore — Verifying Binaries / Rekor transparency log](https://docs.sigstore.dev/logging/verify-release/)
- [RFC 9711 — The Entity Attestation Token (EAT)](https://www.rfc-editor.org/info/rfc9711/)
- [CCxTrust: Confidential Computing Platform Based on TEE and TPM Collaborative Trust — arXiv:2412.03842](https://arxiv.org/pdf/2412.03842)
- [Trustless Attestation Verification With Zero-Knowledge Proofs — TikTok for Developers](https://developers.tiktok.com/blog/verifying-trusted-execution-environments)
