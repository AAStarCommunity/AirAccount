# AirAccount KMS — Feature Update

**日期**: 2026-06-01（最后更新：2026-06-01）
**版本跨度**: v0.17.0 → v0.19.0  
**分支**: KMS / feat/grant-session-v2  
**相关 PR**: #9 (EIP-712, merged), #13 (P256 session key, open), #19 (grant-session, open)

---

## 一、四个里程碑 Feature 概述

### 1. v0.17.0 — Agent Key 子系统（secp256k1 BIP-44，TEE 派生）

**新增 API**：
- `POST /kms/create-agent-key` — 在 TEE 内派生 secp256k1 子密钥，颁发 JWT 凭证
- `POST /kms/sign-agent` — 持 JWT 对 userOpHash 签名（106 字节格式，含 `0x08` marker）
- `POST /kms/refresh-agent-credential` — JWT 轮转，旧凭证立即失效
- `POST /kms/revoke-agent-credential` — 吊销 Agent Key，需要 WebAuthn 确认

**架构要点**：
```
用户 WebAuthn 一次（create-agent-key）
     ↓
TA 派生 m/44'/60'/0'/1/<index>（secp256k1）
TA 颁发 JWT（HMAC-HS256，密钥在 TEE 安全存储）
     ↓
SDK 持有 JWT，调用 sign-agent(JWT, userOpHash)
     ↓
TA 返回：[0x08][account(20)][key(20)][ECDSA(65)] = 106 字节
```

**关键约束**：
- `create-agent-key` **强制要求** WebAuthn ceremony（带 challenge 防重放）；raw passkey assertion 被拒绝
- 钱包必须已存在（即 AirAccount 用户，TEE 内有私钥）
- 每个 credential 有独立速率限制和 `credential_hash`，轮转后旧 hash 立即失效
- `refresh-agent-credential` / `revoke-agent-credential` 同样要求 WebAuthn（`purpose="authentication"`）

---

### 2. v0.17.2 — Session Key 签名格式升级（issue #7）

**背景**：合约 v0.17.2 要求签名包含 `account` + `key` 地址，以便 validator 无需链上查表。

**签名格式变化**：

| 版本 | secp256k1 格式 | P256 格式 | 说明 |
|------|----------------|-----------|------|
| v0.17.0 | `[0x08][ECDSA(65)]` = 66 字节 | — | 合约需链上查 key 地址 |
| v0.17.2 | `[0x08][account(20)][key(20)][ECDSA(65)]` = **106 字节** | `[0x08][account(20)][keyX(32)][keyY(32)][r(32)][s(32)]` = **149 字节** | 合约一次解包，无链上查表 |

**TA 变化**：`sign_agent_user_op` 由私钥推导 `key` 地址，CA 提供 `account`，TA 在 TEE 内组装完整格式。SDK 直接将返回字节作为 `UserOp.signature`，无需额外拼装。

---

### 3. v0.18.0 — EIP-712 结构化数据签名（PR #9，已合并）

**新增 API**：
- `POST /kms/sign-typed-data` — 对结构化数据（EIP-712 domain + types + message）做 secp256k1 签名

**架构要点**：
```
POST /kms/sign-typed-data {
  keyId, hdPath, domain, types, message, webAuthnAssertion (必填)
}
     ↓
host 端把 domain/types/message 序列化为 proto 传入 TA
     ↓
TA 内部: EIP-712 digest = keccak256(0x1901 || domainSeparator || hashStruct(message))
TA 用 HD 私钥签名 → 65 字节 recoverable sig（R||S||V，V=27/28）
```

**安全变更（v0.19.0 含修复）**：
- `sign-typed-data` 现在**强制要求** WebAuthn ceremony（`webAuthnAssertion` 必填），legacy passkey assertion 被拒绝
- 内部使用 `resolve_grant_passkey_assertion(..., "authentication")`，只接受 `purpose="authentication"` 的 challenge，`grant-session` challenge 无法跨用于此操作

**当前限制**：
- EIP-712 支持 `address`、`uint*`、`int*`、`bool`、`bytes32`、`string`、`bytes` 类型；数组和 `bytes1`～`bytes31` 定长字节 defer 至后续版本

---

### 4. v0.19.0 — Grant Session 链下签名（PR #19，open）

**背景（Tasks 3+4）**：Owner 钱包在 TEE 内签署 `GRANT_SESSION_V2` / `GRANT_P256_SESSION_V2` 授权哈希，relayer 代为提交链上 `grantSessionWithSig` / `grantP256SessionWithSig`。Owner 零 Gas，签名在 TEE 内生成，私钥不出 TEE。

**新增 API**：

| 端点 | 方法 | 说明 |
|------|------|------|
| `GET /kms/begin-grant-session-auth?keyId=<uuid>` | GET | 颁发 `purpose="grant-session"` 的 WebAuthn challenge |
| `POST /kms/sign-grant-session` | POST | Owner 对 ECDSA session key 授权哈希签名 |
| `POST /kms/sign-p256-grant-session` | POST | Owner 对 P256 session key 授权哈希签名 |

**`POST /kms/sign-grant-session` 请求体**：
```json
{
  "keyId": "<wallet-uuid>",
  "hdPath": "m/44'/60'/0'/0/0",
  "chainId": 10,
  "verifyingContract": "0x<SessionKeyValidator 地址，20 字节>",
  "account": "0x<智能账户地址，20 字节>",
  "sessionKey": "0x<secp256k1 session key 地址，20 字节>",
  "expiry": 1780000000,
  "contractScope": "0x0000000000000000000000000000000000000000",
  "selectorScope": "0x00000000",
  "velocityLimit": 0,
  "velocityWindow": 0,
  "callTargets": [],
  "selectorAllowlist": [],
  "nonce": 0,
  "webAuthnAssertion": { "challengeId": "...", "credential": "..." }
}
```

**`POST /kms/sign-p256-grant-session` 请求体**（`sessionKey` 替换为 `keyX`/`keyY`）：
```json
{
  "keyId": "<wallet-uuid>",
  "keyX": "0x<P256 pubkey X，32 字节>",
  "keyY": "0x<P256 pubkey Y，32 字节>",
  ... 其余字段同上 ...
}
```

**返回**：
```json
{
  "keyId": "<wallet-uuid>",
  "signature": "0x<65 字节 hex，R(32)||S(32)||V(1)，V=27 或 28>"
}
```

**链上对应合约函数**：
```solidity
// SessionKeyValidator.sol
function grantSessionWithSig(address sessionKey, SessionConfig cfg, bytes calldata ownerSig) external
function grantP256SessionWithSig(bytes32 keyX, bytes32 keyY, SessionConfig cfg, bytes calldata ownerSig) external
```

**哈希构造（TEE 内，与合约精确匹配）**：
```
inner = keccak256(abi.encode(
  "GRANT_SESSION_V2",          // string — dynamic type，offset = 13*32 = 416 字节
  block.chainid,               // uint256
  address(this),               // address（SessionKeyValidator）
  account,                     // address
  sessionKey,                  // address
  cfg.expiry,                  // uint256
  cfg.contractScope,           // address
  cfg.selectorScope,           // bytes4（右对齐，ABI 编码）
  cfg.velocityLimit,           // uint256
  cfg.velocityWindow,          // uint256
  keccak256(abi.encodePacked(cfg.callTargets)),       // bytes32
  keccak256(abi.encodePacked(cfg.selectorAllowlist)), // bytes32
  grantNonces[account][sessionKey]                    // uint256（nonce，链上读取）
))
// 总编码：480 字节（头部 416 + string 长度 32 + string 数据 32）

final = keccak256("\x19Ethereum Signed Message:\n32" || inner)
// 即 OpenZeppelin toEthSignedMessageHash(inner)
```

P256 版本：14 个字段（多 `keyX`/`keyY`，无 `sessionKey`），头部 448 字节，总 512 字节。

**安全约束**：
- 必须先调用 `GET /kms/begin-grant-session-auth` 获取 challenge（非 `begin-webauthn-auth`）
- challenge 的 `purpose` 必须为 `"grant-session"`；`"authentication"` challenge 无法用于此操作
- `expiry` 超过 `uint48` 最大值（`281474976710655`，约 Year 10889）时返回 400
- challenge 消费后立即删除（单次有效，防 replay）
- `nonce` 需在调用前从链上读取 `grantNonces[account][sessionKey]`

---

## 二、用户认证体系（v0.19.0 全量）

| 操作 | 需要 AirAccount 用户 | 需要 WebAuthn | Challenge Purpose | 频率 |
|------|---------------------|--------------|-------------------|------|
| create-agent-key | ✅ 必须 | ✅ 强制 | `authentication` | 创建时一次 |
| sign-agent | ✅ 通过 JWT 绑定 | ❌（JWT 鉴权） | — | 每次签名 |
| refresh-agent-credential | ✅ 必须 | ✅ 强制 | `authentication` | 轮转时一次 |
| revoke-agent-credential | ✅ 必须 | ✅ 强制 | `authentication` | 吊销时一次 |
| sign-typed-data | ✅ 必须 | ✅ 强制 | `authentication` | 每次签名 |
| create-p256-session-key | ✅ 必须 | ✅ 强制 | `authentication` | 创建时一次 |
| sign-p256-user-op | ✅ 通过 JWT 绑定 | ❌（JWT 鉴权） | — | 每次签名 |
| revoke-p256-session-key | ✅ 必须 | ✅ 强制 | `authentication` | 吊销时一次 |
| **sign-grant-session** | ✅ 必须 | ✅ 强制 | **`grant-session`** | 每次授权 |
| **sign-p256-grant-session** | ✅ 必须 | ✅ 强制 | **`grant-session`** | 每次授权 |

> **Challenge Purpose 隔离**（v0.19.0 起完整实现）：
> - `begin_webauthn_auth` → `purpose="authentication"` → 通用操作（sign/delete/change_passkey 等）
> - `begin_grant_session_auth` → `purpose="grant-session"` → 仅 sign-grant-session / sign-p256-grant-session
> - 服务端在消费 challenge 时强制检查 purpose，跨用途立即返回 400

---

## 三、合约侧签名类型速查

| 场景 | KMS 端点 | 格式 | Marker | 链上用途 |
|------|---------|------|--------|---------|
| Agent UserOp（secp256k1）| `sign-agent` | 106 字节 | `0x08` | `UserOp.signature` |
| Agent UserOp（P256）| `sign-p256-user-op` | 149 字节 | `0x08` | `UserOp.signature` |
| Grant ECDSA session key | `sign-grant-session` | 65 字节 EIP-191 | 无 | `grantSessionWithSig.ownerSig` |
| Grant P256 session key | `sign-p256-grant-session` | 65 字节 EIP-191 | 无 | `grantP256SessionWithSig.ownerSig` |
| 任意 EIP-712 数据 | `sign-typed-data` | 65 字节 EIP-712 | 无 | 通用结构化签名 |

---

## 四、Session Key 到期后的处理

当前行为：JWT 过期后拒绝签名，但 DB 行和 TEE secure storage 对象**不自动清理**。

**TEE 存储估算**（每个 P256 session key ~400 字节）：
- OP-TEE secure storage 通常 4-16 MB
- 高频创建场景约 10,000-40,000 个 key 后爆仓

**待修复**（见七节 P2 跟进事项）。Agent Key（secp256k1）使用 BIP-44 无状态派生，不在 TEE 单独存储，无此风险。

---

## 五、SDK 使用示例

### Grant Session 链下签名（v0.19.0 新增）

```typescript
// 1. 获取 grant-session 专属 challenge（不能用 begin-webauthn-auth 的 challenge）
const { challengeId } = await fetch(
  `/kms/begin-grant-session-auth?keyId=${walletId}`
).then(r => r.json());

// 2. 用户 WebAuthn 认证（使用上面的 challenge）
const credential = await navigator.credentials.get({ /* challenge from Step 1 */ });

// 3. TEE 内签名
const { signature } = await fetch('/kms/sign-grant-session', {
  method: 'POST',
  body: JSON.stringify({
    keyId: walletId,
    chainId: 10,
    verifyingContract: SESSION_KEY_VALIDATOR_ADDR,
    account: smartAccountAddr,
    sessionKey: ecdsaSessionKeyAddr,
    expiry: Math.floor(Date.now() / 1000) + 86400 * 7,
    contractScope: "0x0000000000000000000000000000000000000000",
    selectorScope: "0x00000000",
    velocityLimit: 0, velocityWindow: 0,
    callTargets: [], selectorAllowlist: [],
    nonce: await getGrantNonce(account, sessionKey),  // 链上读取
    webAuthnAssertion: { challengeId, credential }
  })
}).then(r => r.json());

// 4. Relayer 提交（owner 零 Gas）
await sessionKeyValidator.grantSessionWithSig(sessionKey, sessionConfig, signature);
```

### Agent Key（自主型，v0.17.0+）

```typescript
// 创建（用户在场一次）
const agentKey = await airAccount.createAgentKey({
  humanKeyId: walletId,
  webAuthnAssertion: await webauthn.authenticate()
});

// 后续自动签名（返回 106 字节 hex）
const sig106 = await airAccount.signAgent({
  keyId: agentKey.keyId,
  bearer: agentKey.agentCredential,
  payload: userOpHash
});
```

### P256 Session Key（助理型，v0.17.2+）

```typescript
// 创建（用户在场一次）
const p256Key = await airAccount.createP256SessionKey({
  humanKeyId: walletId,
  webAuthnAssertion: await webauthn.authenticate()
});

// 后续自动签名（返回 149 字节 hex）
const sig149 = await airAccount.signP256UserOp({
  keyId: p256Key.keyId,
  bearer: p256Key.agentCredential,
  payload: userOpHash,
  accountAddress: smartAccountAddr
});
```

### EIP-712 结构化数据签名（v0.18.0+，v0.19.0 起 WebAuthn 强制）

```typescript
// 必须用 begin-webauthn-auth（非 begin-grant-session-auth）
const { challengeId } = await fetch(
  `/kms/begin-webauthn-auth?keyId=${walletId}`
).then(r => r.json());

const credential = await navigator.credentials.get({ /* challenge */ });

const { signature } = await fetch('/kms/sign-typed-data', {
  method: 'POST',
  body: JSON.stringify({
    keyId: walletId,
    domain: { name: "MyApp", version: "1", chainId: 10, verifyingContract: "0x..." },
    types: { Order: [{ name: "amount", type: "uint256" }, { name: "to", type: "address" }] },
    message: { amount: "1000000000000000000", to: "0x..." },
    webAuthnAssertion: { challengeId, credential }
  })
}).then(r => r.json());
```

---

## 六、开放 Issue 状态（2026-06-01）

| Issue | 标题 | 优先级 | 状态 |
|-------|------|--------|------|
| #19 | grant-session off-chain signing (Tasks 3+4) | **P1** | open，5 轮 Codex 审查通过，等待 review |
| #13 | P256 session key (create/sign/revoke) | **P1** | open，等待合并 |
| #11 | [Contract v0.17.2] grantSessionDirect tightened — UserOp self-call no longer accepted | **P1** | 影响 session key 合约集成，待对齐 |
| #10 | [FYI] SuperPaymaster v5.3.3 breaking change | P3 | 仅 FYI，无 KMS 改动 |
| #6 | kms: align with SuperPaymaster v5.3.3-beta — EIP-712 + UserOp v0.7 | P2 | EIP-712 已完成，UserOp v0.7 为 SDK 侧问题 |

---

## 七、待跟进事项

| 优先级 | 事项 | 状态 | 负责方 |
|-------|------|------|-------|
| P1 | sign-typed-data 强制 WebAuthn | ✅ v0.19.0 已修复 | — |
| P1 | grant-session challenge purpose 隔离（5 轮 Codex 审查）| ✅ v0.19.0 已修复 | — |
| P1 | 与合约团队确认 #11 grantSessionDirect 新路径 vs PR #13/19 格式兼容性 | 待对齐 | KMS + 合约 |
| P2 | P256 session key TEE 存储 GC 机制（过期自动清理或定期清理）| 待开 issue | KMS |
| P2 | 每钱包 P256 session key 数量上限保护 | 待开 issue | KMS |
| P2 | `complete_registration` 未检查 challenge purpose（DoS/不变量漂移）| 待开 issue | KMS |
| P3 | sign_count 更新错误静默忽略（影响认证器克隆检测）| 待开 issue | KMS |
| P3 | SDK 新增：grant-session 端点 + begin-grant-session-auth | 待 PR | SDK |
| P3 | Issue #6 UserOp v0.7 paymaster 字段 | 无 KMS 改动 | SDK |
