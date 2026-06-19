# Guardian 签名的 KMS 依赖 · DR/信任保证 · 迁移到纯客户端 P-256

> 类型：风险/设计评估（Risk/Design）
> 触发：issue #102（KMS 侧）+ airaccount-contract#119（合约侧主力）+ YetAnotherAA#311（独立性主线）
> 日期：2026-06-19
> 关联：`memory: project_decentralization_model`（半去中心化：换实例=换 rpId=social recovery）

---

## 0. 结论先行

- AA 账户**主签名**（P-256 owner passkey，经 KMS）**继续用 KMS，没问题**——不在本文范围。
- 账户**guardian（社恢复）槽位**若依赖「KMS 托管 secp256k1 私钥替 guardian 签」= **单点信任**，且**与社恢复的目的自相矛盾**（见 §3 循环依赖）。
- **正确方向**：guardian 槽位走**纯客户端 P-256（WebAuthn passkey）**，链上用 **RIP-7212 P-256 precompile/verifier** 验证（合约 #119 主力）。guardian 的 passkey 本身就是钥匙（设备 + iCloud/Google 同步），**KMS 不在 guardian 签名链路**。
- **KMS 侧代码工作量：零。** KMS 没有 guardian 专用代码（它只是被集成方当通用 `CreateKey(secp256k1)+Sign` 签名器借用）。「移出链路」= 集成方不再这么用，KMS 不需要改/删任何东西。本文是 #102 要求的 **DR/信任澄清 + 去依赖配合**。

---

## 1. 现状：guardian 为什么会扯上 KMS

- 账户合约 T3 恢复格式末段是 `guardianECDSA(65)`，链上用 `ecrecover` 验证 → **要求 secp256k1 签名**。
- 普通人当 guardian 用的是 **passkey（P-256）**，passkey **产生不了 secp256k1 签名**。
- 于是 passkey-guardian 的 ECDSA 只能「借道」：guardian 在 KMS 建一把 secp256k1 key、用 passkey 作授权门，**真正的签名由 KMS 出**，结果填进 `guardianECDSA`。
- ⚠️ 注意：这是**集成方的用法**，不是 KMS 的专用功能——KMS 代码里没有任何 `guardian` 端点，它只是被复用了通用签名能力。

---

## 2. KMS 托管密钥的 DR / 可用性 / attestation 保证（回答 #102 诉求 1）

给集成方一个**可推理的信任模型**——诚实陈述，不粉饰：

| 维度 | 现实 |
|---|---|
| **密钥位置** | OP-TEE 安全存储，加密密钥源自 **HUK（硬件唯一密钥）** → 密钥**绑定到那一块 MX93 板子**。 |
| **跨实例备份/恢复** | **无。** 密钥不出 TEE、HUK 不可导出 → 一块板子上创建的 key**无法在另一块板子重建**。 |
| **板子损坏/丢失** | **密钥永久丢失。** 没有 DR。换板子 = 新 HUK = 新密钥 = **同一地址签不出来了**。 |
| **切换实例** | 见上：换实例 = 换密钥 = 换地址（与「换实例=换 rpId=social recovery」一致）。**不能用同一地址继续签。** |
| **attestation 之外的约束** | **几乎没有。** KMS 持有那把 secp256k1 私钥；passkey 授权门由**同一个 KMS** 执行。所以一个被攻陷/作恶的 KMS **在技术上可以**替 guardian 签一笔恶意恢复（把 owner 改成攻击者）。TEE attestation 只能证明「跑的是预期 TA」，**不能约束 operator 不滥用它持有的密钥**。 |

**一句话信任模型**：把 guardian 押在 KMS 上 = 信任（TEE 完整性）+ 信任（operator 不作恶/不下线）= **单点**，且**无 DR**。

---

## 3. 核心矛盾：guardian 依赖 KMS 与社恢复的目的自相矛盾

社恢复（guardian）存在的意义，恰恰是**当你失去对账户的主要控制时把账户救回来**。AirAccount 的去中心化模型里，「换实例/换 rpId」本身就要靠 social recovery 兜底。

- 如果 **guardian 又依赖 KMS**，而 KMS 正是你可能要「恢复脱离」的那个中心点 →
- **KMS 挂了 / 跑路 / 作恶时，guardian 也签不出 / 被冒签** → 社恢复在最需要它的时刻失效。
- 而且 passkey 没丢、用户也救不回这个 guardian（passkey 重建不了那把 secp256k1）。

**这是循环依赖**：用「依赖 KMS 的东西」去给「脱离 KMS」兜底。纯客户端 P-256 guardian 打破这个环。

---

## 4. 推荐方向（回答 #102 诉求 2）：guardian → 纯客户端 P-256

| | KMS 托管 secp256k1 guardian（现状借用） | 纯客户端 P-256 passkey guardian（#119 方向） |
|---|---|---|
| 钥匙在哪 | KMS 板子的 TEE | guardian 自己的设备 + iCloud/Google 同步 |
| DR / 可恢复性 | 无（板毁即失） | 强：换设备只要登回 Apple/Google 账户，passkey 自动同步回来 |
| 单点 | 是（KMS） | 否（去中心化，每个 guardian 自持） |
| 恶意 operator 可冒签 | 可以 | 不能（链上 RIP-7212 验 guardian 自己的 P-256 签名） |
| 普通人可用性 | 需懂 KMS | 极好：有手机就行（自己两台手机 / 老婆 / 朋友各一台） |
| 信任假设 | 信 TEE + 信 operator | 信 Apple/iCloud、Google 的 passkey 同步（业界主流、可推理） |

**KMS 是否有必须留在 guardian 链路的理由？没有。** guardian 完全可以走纯客户端 P-256；KMS 只服务**账户主签名**等其他场景。

---

## 5. 落地顺序与各方职责

```
1. 合约 #119（主力，airaccount-contract 仓库）
   guardian 槽位支持 P-256(WebAuthn) 签名，链上 RIP-7212 验证。  ← 真正的代码改动在这
2. KMS #102（本仓，配合）
   = 本文档（DR/信任澄清）+ 表态：KMS 不需留在 guardian 链路、零代码改动。
   ⚠️ 不要在 #119 落地前移除/改动任何「通用 secp256k1 签名」能力——
     否则会破坏现有「KMS 借道当 guardian」的账户的恢复路径。
3. YAA（YetAnotherAA）
   #119 落地后，才能在 UI 加「纯 passkey guardian」作为第 4 个渠道。
   现在 UI 不列它为可用 = 正确。
```

**KMS 侧本期交付**：本文档 + #102 上的评估意见。**无 KMS 代码改动、无 KMS 版本发布**——可发布的代码工作在合约 #119。
