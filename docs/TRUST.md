<!-- Created: 2026-06-16 -->
# AirAccount 信任模型 / Trust & Verifiability

> 这是 AirAccount「为什么可信、怎么验证」的**总文档**。把分析、判断、机制、运维一处讲清。
> 细节文档：[`attestation-trust-root-decision.md`](./design/attestation-trust-root-decision.md)（NDA + 替代路径调研）· [`measurement-provenance-design.md`](./design/measurement-provenance-design.md)（透明日志 + 链上注册表设计）· [`transparency-log-ops.md`](./design/transparency-log-ops.md)（运维）· [`threat-model-ca-adversary.md`](./design/threat-model-ca-adversary.md)（威胁模型）· [`security-roadmap.md`](./design/security-roadmap.md)

---

## 一句话（人话版）

> **你不用"相信 AAStar 不作恶"。AirAccount 把"我在跑哪个 TEE 程序"这件事，公开钉死在一个谁都改不了、谁都能查的公共日志里——AAStar 想偷偷换成有后门的版本，做不到，且立刻会被发现。再加上代码开源可自己复算、关键操作还要独立第三方共签，三道一起，把"信任一家公司"降级成"信任公开的数学和记录"。**

---

## 1. 我们到底在保护什么

AirAccount 是 TEE 私钥管理：私钥在硬件可信执行环境（OP-TEE）里生成、永不出来，每次签名要一次实时防重放的 WebAuthn 验证。威胁模型最硬的对手是 **V5：运行 KMS 的服务器（CA）被完全攻陷，甚至 AAStar 自己变坏**。问题：你凭什么相信"它真的在跑那个安全的、开源的 TA，而不是一个偷偷改过的后门版本"？

这正是**远程证明（remote attestation）**要回答的：*跑的是什么代码、在哪、什么时候*。

## 2. 信任根的三个来源（我们的判断）

远程证明的信任锚本质只有三家：

| 家族 | 怎么建立信任 | AirAccount |
|---|---|---|
| **(A) 厂商硬件 PKI** | 信芯片厂商（NXP/Intel…）的出厂证书链 | ⛔ NXP NDA 对个人申请被拒（Case #00987060，需法律实体）；且只证"正宗芯片"、未必有可离线验的链 |
| **(B) 可复现 + 透明** | 开源可复现构建 + 公开防篡改日志，**人人可自查** | ✅ **已实现并上线** |
| **(C) 去中心化 / 经济** | 门限独立共签 + 链上注册 + 质押罚没，**信任来自数量与激励** | ✅ DVT 门限共签（#70）已交付 |

**判断（[决策文档](./design/attestation-trust-root-decision.md)）**：NXP 路线（A）卡 NDA、且对一个半去中心化的开源 KMS 不是关键路径——它影响信任的"天花板"，不影响"地板"。**所以信任根战略定为 (B) + (C) 为主、(A) 为可选增强**。这也正是 Web3 attestation 的行业趋势（"attestation 是信号，不是信任模型"——真正的信任来自共识 + 透明，而非厂商根）。

## 3. (B) 透明日志：现在它解决了什么

### 原来的缺口
"我在跑哪个 TA"由一份 **measurement 清单（manifest）** 声明，AAStar 一把私钥签名。**这把钥匙若泄露或被胁迫**，攻击者能签一份把后门 TA 列为"正常"的清单，**单独发给某个受害者**，对方分辨不出。

### 透明日志补的（问责制，和 Certificate Transparency 同源）
AAStar 签过的**每一份清单都必须进一个公开 append-only 日志**（多个独立见证人共签，保证日志不能对不同人撒不同的谎）：
- **偷偷投毒做不到**：恶意清单要么进了公开日志（人人可见、监控会抓到），要么过不了客户端校验。
- **信任转移**：从"相信这把钥匙永不被滥用" → **"AAStar 改不了已公开承诺的东西，任何滥用都公开可查、会被发现"**。

> ⚠️ 诚实边界：透明日志给的是**问责与可检测**，不是"阻止"。它**不**单独证明代码无恶意（那靠**可复现构建**：任何人用公开源码重算 measurement 比对）、**不**锚定 NXP（那是 R-1，B 线 Phase 2，卡 NDA）。三件事叠起来才完整：**可复现 ⊕ 透明日志 ⊕ DVT 独立共签**。

### 外部用户怎么验证（三步）
```
1. GET /attestation?nonce=<随机>                          → evidence（含 ta_measurement）
2. GET /.well-known/attestation-measurements.json         → 签名清单
   GET /.well-known/attestation-measurements-proof.json   → 该清单的 Sigsum 透明日志证据
3. 用 @aastar/attestation-verifier 验：
   - 清单签名（pin 的发布者公钥）
   - 清单在公开日志里、≥quorum 见证人共签（Tier-2）
   - 证据绑定这份清单（防张冠李戴）
   - evidence 的 ta_measurement ∈ 清单里 current/未吊销的集合
   - （可选）用 scripts/ta-measurement.sh 从开源源码重算 measurement，确认 == 清单值
```
**得到的保证**：正在跑的 TA，其 measurement 是一个**被公开钉死、且能重算回开源代码**的值。

## 4. 运维：不给运行时加任何负担

**关键：发布日志是"发版时"的动作，不是运行时。** 清单只在 measurement 变（=发新版 TA）时才变。

- 运行时（板子/KMS 进程）：**只静态服务两个文件**（清单 + 证据），**不连日志、不加常驻进程、不加开机 hook**。
- 发版时（CI）：自动把 `SHA-256(清单)` 提交公共日志、收齐见证人共签、产出证据文件、随版本发布。
- **B-4 监控**：一个定时（每 6h）GitHub Action，拉公网 `kms.aastar.io` 的清单+证据实时复验、并比对仓库源，发现被换/未登记/验不过就告警。**也是定时任务，非常驻。**

详见 [运维方案](./design/transparency-log-ops.md)。

## 5. 现状（v0.22.0 + 透明日志已上线）

| 能力 | 状态 |
|---|---|
| 私钥不出 TEE + WebAuthn 强制（V1） | ✅ |
| WebAuthn challenge 绑定下沉 TA、防重放（V2） | ✅（strict feature） |
| payload-commitment 防 CA 偷换待签数据（V4） | ✅ v0.22.0 |
| 远程证明 MVP + 可复现构建 + 签名清单（#37/C 线） | ✅ v0.22.0 |
| **透明日志（B）：清单进公共 Sigsum 日志 + 客户端 Tier-2 验 + 监控** | ✅ **已上线 kms.aastar.io**，对公共日志 `test.sigsum.org/barreleye` 端到端验通 |
| DVT 独立门限共签缓解 V5（C 维度，#70） | ✅ 绑定向量随 v0.22.0 |
| 链上 measurement 注册表（C 升级，#88） | 📋 规划 |
| 锚定 NXP 硬件根（A，R-1，#13/#48） | ⛔ 卡 NDA（个人被拒，走法律实体） |

**结论**：AirAccount 当前是一个**诚实、可独立验证的半去中心化信任模型**——你能密码学验证它在跑什么、自己重算代码、查公开日志、且关键操作有独立共签。不依赖"相信 AAStar"，也不假装拿到了它没有的 NXP 背书。

---

## 6. 硬件安全根基（enclave）—— 让"私钥不出 TEE"变成硬件强制

上面（软件层 + 透明 + 去中心化）解决"对外可信"。底下还有一层**芯片内置的硬件安全机制（TrustZone TEE + NXP EdgeLock Enclave/ELE）**，让"私钥不出 TEE"不是一句口号、而是**硬件强制**。这六条是**客观、可核**的信任特征：

| # | 硬件机制 | 给 KMS 的价值 | 现状 |
|---|---|---|---|
| 1 | **TrustZone 隔离** | key 操作跑在 secure world，富 OS（Linux）哪怕被 root 也读不到 key | ✅ 已有（地基） |
| 2 | **硬件绑定加密存储** | key 用硬件派生密钥（HUK→SSK→TSK→FEK，OP-TEE 默认层级）加密落盘，eMMC 拔走到别的机器解不开 | ✅ 已有（用 OP-TEE secure storage） |
| 3 | **RPMB 防回滚** | 防止把存储回退到旧状态（恢复已删 key、重放旧 nonce/计数器） | ⏳ 代码已写，待烧 RPMB key 开启（#50） |
| 4 | **硬件 TRNG（ELE）** | 真随机生成 key/nonce，非软件伪随机 | ✅ 已用（ele-trng） |
| 5 | **Secure Boot（AHAB/ELE）** | 开机就验 bootloader/TEE/TA 未被篡改才运行 | ⏳ 开发板默认未强制（lifecycle 开放）；生产可烧 SRK + 闭锁启用 |
| 6 | **ELE 硬件 crypto** | AES/HMAC/SHA/NIST-P256/Ed 可硬件加速并让密钥留在硬件内（secp256k1 不支持，留软件） | ⏳ 当前全软件，可逐步卸载 |

> 这六条不依赖 NXP 的 NDA 背书（那是另一层、我们不做）——它们是**这块芯片本来就提供、我们用起来**的硬件安全能力。

### PSA 是什么 / 为什么提它
**PSA = Arm Platform Security Architecture**（Arm 平台安全架构）——Arm 推的一套**硬件+固件安全标准框架**，把一个可信平台"该有的安全服务"规范成四件套：**安全存储 + 加密 + 远程证明（attestation）+ 安全启动（secure boot）**，合称 "PSA 信任根"，并有 **PSA Certified L1/L2/L3** 第三方认证、以及标准化的 attestation token（RFC 9783）。

对我们的意义：**PSA 是"一个完整 TEE 安全根该具备什么"的业界 checklist + 可信度背书路径**。对照 PSA，我们已经有了安全存储、加密、attestation（#37）、TRNG；待补的是 **RPMB 防回滚（#50）+ Secure Boot**。把安全栈向 PSA 对齐，既是自查清单，也是对外说"我们符合业界标准安全架构"的客观依据。

---

## 7. 委托签名凭证（agent key / P256 session key）的信任模型

AirAccount 除了「每笔都要用户在场」的主签名，还有**委托类凭证**：agent key、P256 session key。它们的模型是 **「铸造时用户在场授权 → TTL 窗口内委托签名」**（类似你登录一次拿一个会话令牌）。要点（更新 2026-06-22）：

**铸造（mint）= 用户在场 + 参数绑定（强）**
- 铸造 `create-agent-key` / `create-p256-session-key` **必须带 WebAuthn 断言，且由 TA 在 TEE 内独立校验**用户在场（#111 / C-1）——被攻陷的 host 也无法无用户铸出凭证。
- **#115**：铸造的 challenge **绑定到 mint 参数**（`index / ttl / subject`，域分离摘要 `SHA-256(nonce ‖ mint_digest)`），strict 下被攻陷的 CA 无法借一次用户手势**篡改铸造参数**（如偷偷延长 ttl）。

**使用（use）= TTL 窗口内委托（有意的便利性折中）**
- 凭证铸出后，在 TTL 窗口内由 **TA 签发的 JWT** 授权签名（`sign_p256_user_op` / agent 签名），**每笔不再要求 passkey**（#117）。这是委托签名的**本意**——用户在铸造时授权了这个会话。
- 边界与缓解：**短 TTL**（默认 3 天，TA 上限 7 天）+ **scope 限制**（grant session 绑 hdPath/目标合约/选择器/速率）+ **可随时撤销**（`RevokeP256SessionKey` / agent revoke）。
- 诚实说明：JWT 是会话凭证，泄漏/被攻陷 holder 可在窗口内签名——这是「便利 vs 每笔在场」的取舍，靠短 TTL + scope + 撤销控制，不是 bug。高敏场景应用更短 TTL / 更窄 scope，或不用委托凭证。

---

## 给开发者：怎么用

```ts
import { verifyAttestation, verifyMeasurementManifest, freshNonceHex } from "@aastar/attestation-verifier";

const nonce = freshNonceHex();
const evidence = await (await fetch(`https://kms.aastar.io/attestation?nonce=${nonce}`)).json();
const manifest = await (await fetch("https://kms.aastar.io/.well-known/attestation-measurements.json")).json();
const proofSidecar = await (await fetch("https://kms.aastar.io/.well-known/attestation-measurements-proof.json")).json();

// Tier-2：清单签名 + 公开日志证据 + 见证人门限 + 绑定
const m = verifyMeasurementManifest(manifest, PINNED_PUBLISHER_KEY, {
  transparency: { proof: proofSidecar.proof, policy: SIGSUM_TEST1_POLICY },
});
if (!m.ok) throw new Error("manifest/transparency invalid: " + m.errors.join("; "));

const v = verifyAttestation(evidence, { expectedNonceHex: nonce, expectedMeasurementsHex: m.measurementsHex });
if (!v.ok) throw new Error("attestation invalid: " + v.errors.join("; "));
// 通过 → 你在跟一个真 OP-TEE 对话，它跑的 TA 是公开承诺、可复算的版本。
```
