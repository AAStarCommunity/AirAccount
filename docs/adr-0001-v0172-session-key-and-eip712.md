# ADR-0001: v0.17.2 兼容性 — Session-Key 签名格式迁移 + EIP-712 原生支持

> 状态: 已采纳  
> 日期: 2026-05-30  
> 关联 Issue: [#6](https://github.com/AAStarCommunity/AirAccount/issues/6) [#7](https://github.com/AAStarCommunity/AirAccount/issues/7)  
> 目标版本: v0.18.0

---

## 1. 背景

### 1.1 Issue #7 — Contract v0.17.2 Breaking Change

`airaccount-contract v0.17.2` 将 `AgentSessionKeyValidator`（ASK）合并进统一的 `SessionKeyValidator`，签名字节布局同步变化：

| 版本 | 格式 | 字节数 |
|------|------|--------|
| v0.17.0（当前） | `[0x08][ECDSA(65)]` | 66 bytes |
| v0.17.2（新）ECDSA | `[0x08][account(20)][key(20)][ECDSA(65)]` | 106 bytes |
| v0.17.2（新）P256 | `[0x08][account(20)][keyX(32)][keyY(32)][r(32)][s(32)]` | 149 bytes |

`account` = 调用者 Smart Account 合约地址（由合约在 `validateUserOp` 中用于防跨账户 session-key 滥用）。  
`key` = agent secp256k1 key 的 Ethereum 地址（TA 可从私钥推导）。

> **注意**：当前 KMS `/kms/sign-agent` 返回裸 65 字节 ECDSA，SDK 在外部拼装 `0x08` 前缀。  
> v0.17.2 后 SDK 需要得到完整 106 字节（不含 `0x08`，KMS 内部组装好），因为 `account` 和 `key` 地址必须精确匹配合约验证逻辑。

### 1.2 Issue #6 — EIP-712 原生签名

SuperPaymaster v5.3.3-beta 引入多个新签名流（微支付凭证、GToken EIP-3009、x402 结算），均基于 EIP-712 Typed Data。  
当前 KMS `/Sign` 只接受原始 digest（32 字节），TEE 无法验证"用户实际签了什么"——存在恶意调用方替换 typed data 的风险。

---

## 2. 决策

### 决策 A：Issue #7 — TA 内部组装完整 106 字节签名

**选项对比：**

| | 选项 1（采纳）：TA 组装 | 选项 2：SDK 组装 |
|---|---|---|
| `account` 从哪来 | CA 从请求中读取，传入 TA | SDK 在调用 KMS 后自行拼装 |
| `key` 从哪来 | TA 内从私钥推导（安全） | SDK 需单独拿 agentAddress |
| 安全性 | TA 验证并锁定 account+key | SDK 可伪造 key |
| 接口变化 | `SignAgentRequest` 新增 `accountAddress` | KMS 接口不变 |

**决定**：选项 1。TA 在 TEE 内部用私钥推导 `key` 地址，CA 从请求提供 `account` 地址，TA 组装完整 `[0x08][account(20)][key(20)][ECDSA(65)]` = 106 字节返回。SDK 直接用于 UserOp.signature，不再额外拼装。

本期仅实现 ECDSA（secp256k1）路径；P256 路径在 v0.18 后续实现。

### 决策 B：Issue #6 — 通用 EIP-712 端点 + TA 内部编码

**选项对比：**

| | 选项 1（采纳）：TA 内部全编码 | 选项 2：CA 算 digest，TA 只签 |
|---|---|---|
| 安全性 | TEE 验证类型、域名、字段 | TEE 只见 32 字节 digest，无法审计 |
| 实现复杂度 | 高（需 TA 侧 EIP-712 编码器） | 低 |
| 可审计性 | TA 日志输出 domain.name + primaryType | 无 |

**决定**：选项 1。TA 接受结构化 typed data（domain + types + message），内部完整计算 EIP-712 digest，再用钱包私钥签名。这是唯一能防止调用方伪造 digest 的方案。

**本期范围**：P1（通用 `/kms/SignTypedData` 端点）。P2 预置签名器（Micropayment/GToken/x402）留 v0.18.1。

---

## 3. 技术方案

### 3.1 Issue #7 实现细节

**新增字段（proto）：**
```rust
// proto/src/in_out.rs
pub struct SignAgentUserOpInput {
    pub wallet_id: Uuid,
    pub agent_index: u32,
    pub user_op_hash: [u8; 32],
    pub jwt_kid: String,
    pub jwt_signing_input: Vec<u8>,
    pub jwt_hmac: Vec<u8>,
    pub account_address: [u8; 20],  // NEW: Smart Account address (from request)
}
```

**TA 签名函数输出变化（ta/src/main.rs）：**
```rust
// 当前：65 字节
let mut signature = Vec::with_capacity(65);
signature.extend_from_slice(&sig_bytes);
signature.push(recovery_id.to_i32() as u8 + 27);

// v0.17.2：106 字节  [0x08][account(20)][key(20)][ECDSA(65)]
let agent_address = derive_eth_address_from_secret(&private_key)?;
let mut signature = Vec::with_capacity(106);
signature.push(0x08u8);
signature.extend_from_slice(&input.account_address);
signature.extend_from_slice(&agent_address);
signature.extend_from_slice(&sig_bytes);         // r(32) || s(32)
signature.push(recovery_id.to_i32() as u8 + 27); // V
```

**CA 变化（api_server.rs）：**
```rust
// SignAgentRequest 新增
pub account_address: String,  // "0x..." hex, 20 bytes

// sign_agent() 解析并传入 TA
let account_bytes = parse_address_hex(&req.account_address)?;
let sig_bytes = self.tee.sign_agent_user_op(
    wallet_uuid, agent_index, &user_op_hash,
    jwt_kid, jwt_signing_input, jwt_hmac,
    account_bytes,  // NEW
).await?;
```

### 3.2 Issue #6 实现细节

**EIP-712 编码器（TA 侧）：**

EIP-712 digest = `keccak256(0x1901 || domainSeparator || structHash)`

其中：
- `domainSeparator = hashStruct(EIP712Domain{name, version, chainId, verifyingContract})`
- `structHash = keccak256(typeHash || encodeData(message))`
- `typeHash = keccak256(encodeType(primaryType, types))`

支持的基础类型（v0.18.0 范围）：
- `address` → 32 字节零填充
- `uint256`, `uint128`, `uint64`, `uint32`, `uint16`, `uint8` → 32 字节大端
- `int256`, `int128` → 32 字节大端有符号
- `bytes32` → 直接
- `bool` → 32 字节（0 or 1）
- `bytes` → `keccak256(value)`
- `string` → `keccak256(UTF-8)`
- Struct → 递归 `hashStruct`

不支持（defer）：数组类型、`bytes1`～`bytes31` 定长字节。

**新增 proto 命令：**
```
SignTypedData = 17
```

**新增 API 端点：**
```
POST /kms/SignTypedData
Content-Type: application/json
Authorization: x-api-key

{
  "walletId": "...",
  "domain": {
    "name": "SuperPaymaster",
    "version": "1",
    "chainId": 11155111,
    "verifyingContract": "0x..."
  },
  "types": {
    "Voucher": [
      {"name": "channelId", "type": "bytes32"},
      {"name": "cumulativeAmount", "type": "uint256"}
    ]
  },
  "primaryType": "Voucher",
  "message": {
    "channelId": "0x...",
    "cumulativeAmount": "1000000"
  },
  "webAuthnAssertion": {...}
}
```

---

## 4. 实施计划

### Phase 1 — Issue #7（本次 PR feat/session-key-v0172）

| 步骤 | 文件 | 说明 |
|------|------|------|
| 1 | `proto/src/in_out.rs` | `SignAgentUserOpInput` 加 `account_address: [u8; 20]` |
| 2 | `proto/src/lib.rs` | 更新 roundtrip 测试 |
| 3 | `ta/src/main.rs` | `sign_agent_user_op` 推导 agent 地址，组装 106 字节 |
| 4 | `host/src/ta_client.rs` | `sign_agent_user_op()` 签名加 `account_address` 参数 |
| 5 | `host/src/api_server.rs` | `SignAgentRequest` 加 `accountAddress`，`sign_agent()` 解析传入 |
| 6 | `docs/agent-key-design.md` | 更新签名格式说明 |

### Phase 2 — Issue #6 P1（本次 PR feat/eip712-sign）

| 步骤 | 文件 | 说明 |
|------|------|------|
| 1 | `ta/src/eip712.rs`（新） | EIP-712 编码器（typeHash + encodeData + hashStruct） |
| 2 | `proto/src/in_out.rs` | `SignTypedDataInput / Output` |
| 3 | `proto/src/lib.rs` | `Command::SignTypedData = 17`，roundtrip 测试 |
| 4 | `ta/src/main.rs` | `sign_typed_data()` handler + dispatch |
| 5 | `host/src/ta_client.rs` | `sign_typed_data()` async 方法 |
| 6 | `host/src/api_server.rs` | `SignTypedDataRequest/Response`，`/kms/SignTypedData` 端点 |
| 7 | `host/src/tests.rs` | EIP-712 向量测试（对齐 viem `signTypedData` 输出） |

---

## 5. 风险与缓解

| 风险 | 缓解 |
|------|------|
| EIP-712 编码与 viem/ethers 不完全一致 | 用 4 个 SP v5.3.3 官方向量做 byte-for-byte 测试 |
| TA `account_address` 可被 CA 伪造 | TA 只用于组装签名格式；`account` 由合约验证，TA 负责结构完整性 |
| 新 106 字节格式破坏现有 SDK 集成 | 合约 v0.17.2 未上线前两个版本并行；合约上线后 SDK 统一升级 |
| EIP-712 递归 struct 编码在 TA no_std 环境难实现 | 限制初期范围：仅支持基础类型；struct 嵌套 v0.18.1 再加 |
