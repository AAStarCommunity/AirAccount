# secp256k1 硬件支持分析与采购建议

> 日期：2026-06-12 | 实测平台：NXP FRDM-IMX93（OP-TEE 4.8, ELE LIB 1.1.1）
> 关联 issue：#40（硬件加速）、#48（ELE 私钥存储）、#50（RPMB）

## 结论速览

以太坊钱包用 **secp256k1**（Koblitz 曲线）。经实测 + 大范围调研 + 行业经验三重确认：

- **i.MX 系列内置 ELE（i.MX93 实测确认，i.MX95 架构判断）不支持 secp256k1**，只支持 NIST 曲线（P-256/384/521）+ Ed25519/448 + AES + HMAC + SM4。
- 要硬件 secp256k1，**唯一路径是外接 SE05x 安全芯片**（I2C）。
- 当前软件方案（k256 in OP-TEE TEE）性能良好：**CreateKey ~60ms 端到端**。

## 名词解释（先讲准概念）

- **I2C**（Inter-Integrated Circuit，读作 "I方C" 或 "I-two-C"）：飞利浦 1982 年发明的**两根线串行总线**（SDA 数据线 + SCL 时钟线），用于芯片之间短距离通信。速度：标准 100 kbit/s、快速 400 kbit/s、高速 3.4 Mbit/s。SE05x 就是通过这两根线挂到 i.MX 主控上 —— 简单通用，但带宽低，所以外置安全芯片签名吞吐受 I2C 限制。

- **EAL6+**（Evaluation Assurance Level 6+）：**Common Criteria**（国际通用准则，ISO/IEC 15408）的安全认证等级，从 EAL1（最低）到 EAL7（最高）。EAL6 = "半形式化验证的设计和测试"，"+" 表示额外增强要求。对比：普通商用软件 EAL2-4，银行卡芯片 EAL4-5，**SE05x 这类安全芯片 EAL6+** 属于很高的第三方安全认证级别（接近军工/金融卡）。它衡量的是"安全评估有多严格"，不是性能。

## 当前软件方案性能（实测，i.MX93 localhost 端到端）

| 操作 | 时间 | 说明 |
|------|------|------|
| **CreateKey**（初始化创建钱包：256-bit 熵生成 + 存储 + DB） | **~60ms** | 实测 5 次：49/66/60/60/62ms |
| DeriveAddress（BIP32 派生 + WebAuthn passkey 验证） | ~181ms | 大部分是 p256-m passkey 软件验签，非 secp256k1 本身 |
| SignHash（secp256k1 签名 + WebAuthn passkey 验证） | ~183ms | 同上，纯 secp256k1 签名仅几 ms |

注：派生/签名的 ~180ms 主要花在 WebAuthn passkey 的 P-256 验签（软件 p256-m）上，这部分恰好是 ELE 能硬件加速的（见 #40）。secp256k1 签名本身很快。

## 硬件 secp256k1 方案：外接 SE05x

SE05x 不是"主板内置"，是一颗独立的 I2C 安全芯片，**任何有 I2C 的 i.MX 板子（93/95）都能外接**。

### 选型（要支持 secp256k1）

| 型号 | secp256k1 | 建议 |
|------|:---:|------|
| **SE051**（SE051C） | ✅ 支持（自定义曲线参数化） | **首选**，新一代，EAL6+ |
| **SE050E** | ✅ extended ECC range | 可选 |
| SE050C | ❌ 默认只 NIST P-256 | 不选 |
| SE050F | ❌ FIPS 140-2（不含 secp256k1） | 不选 |

### 开发套件（到手即用，Arduino R3 header + I2C 两种接法）

- **OM-SE051ARD**（SE051 开发板，SE051C2HQ1/Z01XD）← 推荐
- 或 **OM-SE050ARD-E**（SE050E）
- 采购渠道：Mouser / 得捷(DigiKey) / 数字华大等 NXP 代理

### 集成路径（契合 AirAccount 的 TEE 架构）

- SE05x ↔ **I2C** ↔ i.MX 主控
- 通过 **OP-TEE PKCS#11** 接入（NXP Plug & Trust 中间件 + Foundries.io `fio-se05x-cli`）
- 私钥在 SE05x 芯片内生成/签名，**永不出芯片**（EAL6+ 物理隔离）
- 已有 SE050 硬件比特币钱包开源实现可参考

## 决策权衡（采购前必看）

| | 外接 SE051（硬件） | 软件 k256（当前，TEE 内） |
|---|---|---|
| 私钥位置 | SE05x 芯片内，永不出 | OP-TEE secure world 内存 |
| 安全等级 | EAL6+ 物理隔离 | TEE 隔离 + RPMB 防回滚 |
| 创建速度 | 慢（I2C APDU + secp256k1 是自定义曲线非原生，几十~上百 ms） | **~60ms（实测）** |
| 签名吞吐 | 受 I2C 限制，高频场景可能瓶颈 | 够用（纯签名几 ms） |
| BOM / 集成 | +1 芯片 + I2C 走线 + 中间件 | 0 额外成本 |
| 适用场景 | 超高安全、低频（冷钱包级、机构托管、单笔大额） | TEE 私钥管理 + 高频签名（SuperRelay 双签） |

## 建议

1. AirAccount 是 **TEE 私钥管理 + 高频签名**，SE05x 的低吞吐可能拖后腿。软件 k256 + OP-TEE + RPMB 防回滚是更合适的平衡。
2. SE051 适合**超高安全、低频**产品线（如有，值得上）。
3. **采购动作**：先买 1 套 **OM-SE051ARD** 接到现有 i.MX93 板子（I2C），实测 secp256k1 签名延迟和吞吐量，**用真实数据决定量产是否上**——别在 datasheet 上拍板。开发套件几百块，先验证再说。
