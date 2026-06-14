# AirAccount KMS 安全路线图 —— 按威胁模型逐洞补完

> 创建时间：2026-06-14 13:50 +07（本机时间）
> 关联：[`threat-model-ca-adversary.md`](./threat-model-ca-adversary.md)（V1–V5 六向量）· [`37-remote-attestation-design.md`](./37-remote-attestation-design.md) · [`37-attestation-hw-findings.md`](./37-attestation-hw-findings.md) · [`dvt-solution.md`](./dvt-solution.md)
> 目的：把威胁模型里**还没做到位**的攻击向量，拆成可开发、可测试的任务，逐步补完。

---

## 0. 威胁模型现状总览（V1–V5）

| 向量 | 攻击 | 现状 | 缺口 → 任务线 |
|---|---|---|---|
| **V1** | 绕过 passkey | ✅ 已防（验证在 TEE 内，C-1） | — |
| **V2** | 重放 assertion | ⚠️ strict 下已防，但 **legacy 可重放后门仍开** | **A 线**（#63 flip strict）|
| **V3** | 伪造 JWT | ✅ 已防（HMAC key 在 TEE） | — |
| **V3b** | 窃取有效 JWT（bearer） | ⚠️ 部分（bearer token 被盗即可用） | **A 线**（JWT 绑定硬化）|
| **V4** | CA 偷换 payload | ❌ **开放** | **A 线**（#68 payload-bound challenge）|
| **V5** | 假 TEE / 伪造签名环境 | ⚠️ **MVP 已缓解**（#37 Phase 1），未到「不信任部署方」 | **B 线**（#37 Phase 2 / R-1）+ **E 线**（DVT #70）|

> 一句话：**A 线**是主网前的硬门槛（不依赖任何外部资料，马上能做）；**B 线**把 V5 从「半信」推到「真信」（卡 NXP 一手资料 R-1）；**E 线**用门限共签从另一个维度缓解 V5（不卡 R-1）。

---

## A 线 — Challenge 硬化（主网前必做，最高优先级，无外部依赖）

闭合 V2 / V3b / V4。三者要一起上 SDK 才完整。

- **A1 / #63 — strict flip**：`ENFORCE_TA_CHALLENGE=true`，关掉 legacy「无 challenge 绑定」可重放后门；grant-session 也走 TA challenge binding。
  - 开发：TA 侧默认拒绝无 `GetChallenge` nonce 的 assertion；host 配置项默认 strict。
  - 测试：真机 E2E —— 重放旧 assertion 必须被拒；正常 GetChallenge→sign 流程通过。
- **A2 / #68 — payload-bound challenge**：让 passkey 不只证「在场」，还证「签的就是这笔 payload」。
  - 开发：`GetChallenge` 把 payload hash 揉进 nonce/challenge；TA 在 sign 时校验 challenge 绑定的 payload == 实际待签 payload。
  - 测试：CA 偷换 payload（challenge 对、payload 改）必须被 TA 拒绝。
- **A3 / #58 — SDK 升级**：`aastar-sdk` 接 #49 challenge binding 流程（`GetChallenge` + `clientDataJSON`）。mainnet flip strict 前必须，否则客户端发不出合规请求。
  - 测试：SDK 端到端走通新流程；旧流程在 strict 下被拒。

**依赖**：A1↔A3 必须同时发布（strict 一开，旧 SDK 就失效）。A2 可紧随其后。

---

## B 线 — #37 Phase 2：硬件根锚定（把 V5 推到「不信任部署方」）

当前 MVP 信任根是 TOFU（attestation key 设备自签、无 NXP 链）。Phase 2 要连到 NXP 根。

- **B0 / R-1 收口（阻塞项，需一手资料 → 见 §「需要你帮忙拿的资料」）**：确认 ELE `dev_attest` 签名 key 是否 NXP 工厂注入、有无可离线验的 NXP 证书链。**拿不到则 B 线只能停在 TOFU。**
- **B1**：ELE 库内生成 attestation key + `hsm_pub_key_attest` 出证书（需启 **NVM-Daemon** → 要停 KMS 测试窗口，测完恢复，别动生产隧道）。
- **B2**：evidence 携带 ELE `dev_attest` 的 secure-boot 度量（oem_srkh/sha_fw/lifecycle）。
- **B3**：verifier 补 V4 链验（仅 R-1 成立才算可信，否则按降级标注）。
- **R-9**：attestation key 吊销 / 轮换 / 过期机制（绑 RPMB 单调计数器防回滚复活旧 key）。

---

## C 线 — #37 Phase 1.5：工程补强（不依赖 R-1，随时可做）

把 attestation 信任根从「信 AAStar 登记值」升级到「信源码可验」。

- **C1 / R-4 — 可复现构建**：CI 用可复现构建产出 TA，任何人 clone 源码 + 同工具链能 bit-for-bit 重算出同一 `ta_measurement`。
  - 测试：两次独立构建产出同一 measurement；与公布值一致。
- **C2 / §7.1 第 2 档 — 签名 measurement manifest**：发布 `version→ta_measurement` 签名清单到 `kms.aastar.io/.well-known/attestation-measurements.json` + 同步 git tag，客户端独立拉取核对。
- **C3**：`attestation-verifier` 内置已知良好 measurement 列表（多版本并存，滚动升级期）。

---

## D 线 — #37 Phase 3：标准化 + 业务绑定 + 链上验证（按需，靠后）

- **D1**：attestation 绑进 WebAuthn ceremony 和 `SignHash` 响应（默认发生，非可选握手）。
- **D2**：输出 PSA Attestation Token（EAT/RFC 9783，COSE），用 Veraison 做可自部署 verifier（需实测 BSP 是否真暴露 EAT 接口）。
- **D3**：链上 verifier 合约，让 SuperPaymaster / SuperRelay 在链上确认「KMS 跑在真 TEE」。

---

## E 线 — DVT（#70）：门限共签缓解 V5（与 attestation 互补，不卡 R-1）

- **E1**：角色 B co-signer —— BLS 门限共签，防 owner key 单点被盗。增益只来自**独立性**（独立 key + 独立策略 CA 改不了 + 独立通道不经 CA）。
- **E2**：门限 + 节点多样性防单节点作恶；激励绑「正确执行策略」非「签次数」（`ROLE_DVT` + PGL 贡献记录 + slash）。
- 价值：把「单点被盗全损」变「被盗 + 攻破门限才损」。用于大额分层。

---

## 建议执行顺序

1. **PR #72 合入**（#37 Phase 1 MVP，待 jason review）。
2. **A 线**（#63 + #68 + #58）—— 主网前硬门槛，无外部依赖，先打。
3. **C 线**（R-4 可复现构建）与 A 线并行 —— 把 attestation 信任根升级到「源码可验」。
4. **B 线**（R-1）—— 等拿到 NXP 一手资料再攻；拿不到就以 TOFU + C 线为现阶段上限。
5. **E 线**（DVT #70）—— 独立推进，从另一维度缓解 V5。
6. **D 线** —— 标准化 + 链上，靠后按需。
