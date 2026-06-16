<!-- Created: 2026-06-16 -->
# 可验证 Measurement Provenance —— 透明日志 (B) + 链上注册表 (C) 统一设计

> 创建：2026-06-16 +07（本机时间）
> 关联：[`attestation-trust-root-decision.md`](./attestation-trust-root-decision.md)（信任根战略）· [`37-remote-attestation-design.md`](./37-remote-attestation-design.md)（R-4 可复现 measurement + §7.1 manifest）· [`dvt-program-coordination.md`](./dvt-program-coordination.md)（ROLE_DVT / 链上基础设施）
> 目标：用一套**精简、可信、开源**的机制，去掉当前 attestation manifest 的**单一发布者私钥信任点**，把信任根从「信 AAStar 一把 Ed25519 key」升级到「**公开可审计 + 链上治理 + 经济担保**」——不依赖 NXP。

---

## 1. 现状与缺口

v0.22.0 已有：TA 产出可复现的 `ta_measurement`（R-4，bit-for-bit 源码可重算）+ host 服务 `/.well-known/attestation-measurements.json`（**单把 Ed25519 publisher key 签名** + sequence 防降级）+ TS verifier 校验。

**缺口**：信任收敛到**一把发布者私钥**。该 key 若泄露/被胁迫，可对特定受害者签发一份**隐秘的**恶意 manifest（列入被攻陷的 measurement），客户端无从察觉。sequence 只防降级，不防"针对性签发新恶意 manifest"。

**两层解法**（互补，非二选一）：
- **(B) 透明日志**：让发布者签过的**每一份** manifest 都进**公开 append-only 日志** → 隐秘签发不可能（要么公开、要么验不过）。
- **(C) 链上注册表**：把"哪些 measurement 被核准"变成**链上治理 + 经济担保**的多方决定，而非一把 key。

---

## 2. 统一机制：一个 measurement，三档验证

核心思想：**不新增任何 measurement 机制**（沿用 R-4 可复现的 `ta_measurement`），只在其**发布与验证**两端叠加可审计层。客户端按信任/摩擦需求选档，三档结果必须一致。

```
              ta_measurement (R-4 可复现, 已有)
                        │
        ┌───────────────┼────────────────────┐
        ▼               ▼                     ▼
   Tier 1 (现状)    Tier 2 = (B)          Tier 3 = (C)
   Ed25519 manifest  + Sigsum 公开日志      + 链上 MeasurementRegistry
   + sequence 防降级   + 见证人共签           (治理 + 质押/罚没)
   ──────────────    ───────────────       ──────────────────
   防降级,信单key     防篡改,无单key          权威源,经济担保
```

- **链上注册表 (C) = 单一事实源**；**链下签名 manifest (B) = 带透明日志的缓存镜像**（给不读链的客户端）。verifier **交叉校验**：manifest 列表 ⊆ 链上 active 集。
- 同一个 `ta_measurement` 贯穿三档，**无新度量、不动 TA、不动签名热路径**。

---

## 3. Layer B 设计 —— Sigsum 透明日志

**选型：Sigsum**（[sigsum.org](https://www.sigsum.org/)，开源，Go）。理由：极简、专为"公开记录签名校验和以发现 rogue 签名"而生、**见证人共签**（witness 只关心一致性、轻量、可服务多日志）。比 Rekor 更轻、更对口我们的"可复现构建透明"诉求。可自建极简日志（参考 [litetlog](https://github.com/FiloSottile/litetlog)）+ 跑/接入 2–3 个见证人。

**发布**（扩展 `packages/attestation-verifier/scripts/sign-manifest.mjs`）：签好 manifest 后，把其校验和提交到 Sigsum 日志 → 拿到**见证人共签的 tree head + inclusion proof**，随 manifest 一起发布（`.well-known` JSON 内嵌或 sidecar）。

**验证**（扩展 TS `verifyMeasurementManifest`）：在现有 Ed25519 签名 + sequence 之外，**要求合法的 Sigsum inclusion proof + ≥门限见证人共签**（客户端侧 witness policy）。→ 发布者签了但**没公开记录**的 manifest 一律拒；隐秘针对性签发不可能藏。

**监控**（一个 GitHub Action 小 watcher）：盯日志里本 publisher key 名下的条目，与 git-tag 的官方 manifest 集做 diff，异常即告警 → 检测 key 滥用。

**去掉的信任点**：单发布者 key（泄露也藏不住）+ 日志自身分叉（见证人门限）。

---

## 4. Layer C 设计 —— 链上 MeasurementRegistry

**范式：Automata**（把 TEE measurement 锚进链上注册表，已生产验证）。我们更简单：OP-TEE **白名单注册表**，非全 DCAP quote 链上验证。

**合约 `MeasurementRegistry.sol`**（⚠️ **跨仓库：归 airaccount-contract / SuperPaymaster 生态，不在 KMS 仓库**）：
- `mapping(bytes32 measurement => Entry{ status(active/revoked), version, addedAt, taUuid })`。
- **治理（非对称，复用 IPolicyRegistry 模式）**：新增 measurement = **timelock + ROLE_DVT 质押方提议 / DAO 核准**（慢、防误）；吊销 = **即时**（快收紧）。复用 OZ `TimelockController` + SuperPaymaster `ROLE_DVT`。
- **视图**：`isApproved(bytes32) → (bool active, Status)`、`activeMeasurements() → bytes32[]`；变更 emit 事件（链上即透明日志）。
- **经济**：提议方须质押 ROLE_DVT；列入恶意 measurement → 可罚没。复用 SuperPaymaster 质押/罚没。

**验证流**：TS verifier 经 `eth_call` 读注册表（权威档）；SuperPaymaster 可链上 gate。链下 manifest (B) 必须是链上 active 的子集。

**去掉的信任点**：核准从"一把 key"变成**链上多方治理 + 经济担保**。

---

## 5. 实施路径（分阶段，先 B 后 C）

| 阶段 | 内容 | 仓库 | 依赖 |
|---|---|---|---|
| **B-1** | 选定/自建 Sigsum 日志 + 2–3 见证人 | AirAccount（运维）| 无 |
| **B-2** | `sign-manifest.mjs` 提交日志 + 取 proof | AirAccount | B-1 |
| **B-3** | TS verifier 验 inclusion + witness policy | AirAccount | B-2 |
| **B-4** | 监控 GH Action（key 滥用告警）| AirAccount | B-1 |
| **C-1** | `MeasurementRegistry.sol` + 治理（OZ Timelock + ROLE_DVT）| **airaccount-contract（跨仓）** | 无（复用 SuperPaymaster 质押）|
| **C-2** | 发布流写链上 + TS verifier 读链 + 交叉校验 | AirAccount + 合约 | C-1 |
| **C-3** | SuperPaymaster 可选链上 gate | **SuperPaymaster（跨仓）** | C-2 |

**先 B**（纯 AirAccount、不碰链、立即见效）；**后 C**（跨仓、需合约 + 治理）。

---

## 6. 对当前架构的影响

- **B**：纯**加法** —— 新增发布步骤 + verifier 新增一个校验档。**不动 TA、不动 host 签名路径、不动密钥处理**（manifest 已在服务，proof 是 sidecar）。影响：**低**。
- **C**：跨仓合约（airaccount-contract）+ verifier 读链 + 可选 SP gate，复用现有 ROLE_DVT/治理。影响：**中**，且主要在 KMS 之外。
- **两者都不碰 TA / 签名热路径 / 私钥**，都是围绕**已有的可复现 measurement + manifest** 加发布与验证层。**性能零影响。**

---

## 7. 风险评估

| 风险 | 层 | 缓解 |
|---|---|---|
| Sigsum 日志/见证人可用性 | B | ≥门限多见证人；日志不可达时可降级 Tier-1 **带告警**（但重开缺口，属策略选择，默认 fail-closed 到 Tier-1+警告）|
| 运维开销（跑日志+见证人）| B | Sigsum 极简；或接入公共见证人网络；可同时双写 Rekor 增冗余 |
| Sigsum 生态较 Rekor 小 | B | 格式简单；必要时 dual-log |
| 治理被俘获 | C | timelock + 多签/DAO + 非对称即时吊销 |
| 链依赖 / RPC 可用性 | C | Tier-2 链下兜底；verifier 可配置 |
| 合约漏洞 | C | 审计；范围极小（白名单 + 治理）|
| B/C 不一致 | B+C | verifier 交叉校验子集；发布流对账 |

---

## 8. 长期规划（仅列入，非本轮）

- **EAT（RFC 9711）+ Veraison**：标准化 attestation token + 多 endorser verifier。
- **ZK 证明 attestation 有效性**：zkdcap 风格，链上廉价验证、不集中化。
- **TEE + TPM 多根协同**（CCxTrust）、**PUF 内生根 + 链上去中心化验证**。

---

## Sources
- [Sigsum design / witness cosigning](https://git.sigsum.org/sigsum/plain/doc/design.md) · [Sigsum witness.md](https://git.sigsum.org/log-go/plain/markup/witness.md) · [litetlog (FiloSottile)](https://github.com/FiloSottile/litetlog)
- [Keeping Authorities "Honest or Bust" with Decentralized Witness Cosigning — arXiv:1503.08768](https://arxiv.org/pdf/1503.08768)
- [Automata on-chain DCAP attestation](https://github.com/automata-network/automata-dcap-attestation) · [Automata attestation docs](https://docs.ata.network/tee-overview/verifiable-random-function/attestation)
- [Sigstore / Rekor (对比项)](https://docs.sigstore.dev/logging/verify-release/) · [zkdcap](https://github.com/datachainlab/zkdcap)
