# AirAccount KMS — Beta3 计划 & 全局 To-Do

> 创建:2026-06-13 · 上一版本:**Beta2 / v0.20.0 已发布**(tag `v0.20.0` + GitHub Release + `kms.aastar.io/docs` live)

## 0. Beta2 收尾确认 ✅

| 项 | 状态 |
|---|---|
| 代码 PR #51/#35/#33/#2 | ✅ merged |
| Release PR #54(版本+CHANGELOG+文案+banner) | ✅ merged |
| Docs PR #55(OpenAPI+Swagger UI+测试矩阵) | ✅ merged |
| git tag `v0.20.0` + GitHub Release | ✅ |
| `kms.aastar.io` /docs · /openapi.yaml · / dashboard | ✅ live |
| open PR | **0** |
| 真机 E2E 39/39 · 单元 proto 39 + host 56 | ✅ 全绿 |

---

## 1. 全局 To-Do(按轻重缓急,12 个 open issue)

### 🔴 P0 — 主网前必须(安全关键,阻塞 mainnet)
| Issue | 内容 | 备注 |
|---|---|---|
| **#49** | WebAuthn challenge binding(单条 assertion 可重放,H-2) | **Beta3 首发**;#39 信任链最后一块 |
| **#50** | RPMB 生产编程(硬件 anti-rollback,REE-FS fallback 第二阶段) | 需硬件定版后一次性编程(不可逆) |
| **#37** | TEE 远程证明(Attestation)— 客户端验证签名来自真实 OP-TEE | 强烈建议主网前 |

### 🟠 P1 — Beta3 核心(安全加固 + 易用)
| Issue | 内容 |
|---|---|
| **#15** | TA-side JWT exp 检查(用 TEE 时间源,即 v0.20.0 已引入的 `tee_unix_secs`) |
| **#42** | 密钥生命周期管理(active→frozen→pending_delete→deleted) |
| **#52** | P2 便利签名器 host 侧 from/sender == 签名地址 校验(防 EIP-3009 链上 revert) |
| **#21** | EIP-712 domain name 对齐 aastar.io(PaymentPayload 等) |

### 🟡 P2 — 生态对齐
| Issue | 内容 |
|---|---|
| **#6 P3** | SuperPaymaster UserOp v0.7 paymaster + Sepolia E2E(需外部测试环境) |
| **#11** | 合约 v0.17.2 grantSessionDirect 字节级向量验证 |

### 🟢 P3 — 可选 / research / Post-mainnet
| Issue | 内容 |
|---|---|
| **#53** | cla.yml GitHub Action SHA-pin(Low;v0.20.0 已给 Swagger UI 加 SRI,同类) |
| **#40** | CAAM 硬件加速(缩范围:P-256/SHA;secp256k1 软件够用) |
| **#48** | ELE/HSM 私钥存储 research(ELE 当信任根,非 secp256k1 存储) |
| **#38** | PKCS#11 接口层(需先定"哪类 key 可绕过 WebAuthn"红线) |

---

## 2. Beta3(v0.21.0)范围 — 主题:安全加固 + 生态对齐

**纳入 Beta3:**
- **#49** WebAuthn challenge binding ⭐(首发,安全关键)
- **#15** TA-side JWT exp 检查
- **#52** P2 from 校验
- **#21** EIP-712 domain 对齐
- **#42** 密钥生命周期 Phase 1(last_used_at + 状态字段)

**延后:** #50/#37(主网前、需硬件/外部条件)· #6 P3(需 SP 部署地址)· #40/#48/#38(可选)

---

## 3. 首发任务 #49 — WebAuthn challenge binding(设计大纲)

### 问题
TA 的 `verify_passkey_for_wallet` 只验 ECDSA 签名,**不校验 clientDataHash 里的 challenge 内容**。被攻陷的 CA 能用一条捕获的 assertion 重放授权任意 payload。

### 现状
- challenge 由 **host 侧**生成/存储(`db.rs` challenge 表 + `purpose` 字段;`webauthn.rs random_challenge`)。
- TA 完全不知道 challenge —— 只验签名。信任边界停在 host。

### 方案(把 challenge 校验下沉到 TA)
1. **新增 TA 命令 `GetChallenge`**:TA 生成 nonce(TRNG)、存入 TA 内短期状态、返回给 CA。
2. CA 把 nonce 作为 WebAuthn challenge 发给客户端;客户端签名(clientDataJSON 含该 challenge)。
3. **CA 透传完整 clientDataJSON 给 TA**(不再只传 client_data_hash)。
4. TA 内:解析 clientDataJSON → 比对 challenge == 自己发的 nonce → 用后即焚(one-time)→ 再验签名。
5. 时间窗 + nonce 表上限(防 DoS)。

### 影响面
- `proto`:`GetChallenge` 命令 + in/out 结构
- `ta`:nonce 生成/存储/比对 + verify_passkey 改为收 clientDataJSON
- `host`:GetChallenge 透传 + sign 流程改为传完整 clientDataJSON
- 测试:run-full-e2e 加"重放被拒"负向用例(同一 assertion 二次使用 → 拒绝)

### 验收
- 正向:GetChallenge → 签名 → 验证通过
- **负向(核心):同一 assertion 重放第二次 → TA 拒绝(challenge 已消费)**
- 真机 FRDM-IMX93 E2E 通过

---

## 4. 时间线建议
1. **本周**:#49 设计定稿 + proto/TA 骨架 + host 透传(本分支 `feat/beta3-webauthn-challenge-binding`)
2. 之后:#15 / #52 / #21(较小,可并行)→ #42
3. Beta3 发布前:全部真机 E2E + 复用 v0.20.0 的发布流程(版本 bump → CHANGELOG → tag → Release → docs 自动更新)
