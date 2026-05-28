# AirAccount KMS — Agent Key 设计文档

> 分支: `feat/agent-key`  
> 版本: v0.17.0-draft  
> 依据: [issue #3 comment](https://github.com/AAStarCommunity/AirAccount/issues/3#issuecomment-4550954856) + [kms-agent-requirements.md](https://github.com/AAStarCommunity/airaccount-contract/blob/docs/agent-kms-spec/docs/kms-agent-requirements.md)

---

## 0. 背景与定位

AI Agent 需要程序化地代替人类签署 UserOp，现有 KMS 只支持 WebAuthn（生物识别）鉴权。本次在 AirAccount KMS 增加**Agent 身份管理子系统**，三层安全锚：

```
Layer 1 (TEE 硬件):  agentKey 私钥 + kms_secret 封存，永不出 TEE
Layer 2 (JWT 凭证):  Agent 持有 3 天有效 JWT，绑定 keyId，单次可见
Layer 3 (链上 session): callTargets/spendCap/expiry 由人类 WebAuthn 授权后写入合约
```

即便 JWT 泄露，链上 session 兜底（限额 + 即时 revoke）。三层独立，互为保险。

---

## 1. 关键设计决策

### 1.1 签名字节布局（硬约束，逐字节一致）

```
被签哈希 = keccak256("\x19Ethereum Signed Message:\n32" ‖ userOpHash)  // EIP-191, TEE 内强制加
KMS 返回  = R(32) ‖ S(32) ‖ V(1)   // 65 bytes，V 必须归一化为 27 或 28（不是 0/1）
SDK 拼装  = 0x08 ‖ R ‖ S ‖ V        // 66 bytes → UserOp.signature
```

**`/kms/sign-agent` 的 `payload` 字段 = 裸 `userOpHash`（32 字节 hex）**，EIP-191 包裹在 TEE 内做。  
安全要点：KMS 不可裸签外部传入的任意 digest，防被诱导签真实交易哈希。

链上验证：`AgentSessionKeyValidator.validateUserOp` 用 `toEthSignedMessageHash(userOpHash)` recover，
再查 `agentSessions[userOp.sender][recovered]`。

### 1.2 agentKey 类型

- **算法**: secp256k1（链上 `ecrecover`，不是 P256；P256 只用于人类 passkey EIP-7212）
- **派生**: HD BIP44 路径 `m/44'/60'/0'/1/<agentIndex>`，agentIndex 按 **per-humanAccount** 计数
- 每个 humanAccount 下的 agentIndex 从 0 开始，`SELECT COUNT(*) FROM agent_keys WHERE human_account = ?` 得下一个 index
- 优点：同一人类账户下的 agent 密钥可审计，index 语义清晰，BIP44 语义正确

### 1.3 Admin 操作 — CLI 优先，不暴露远程 HTTP

```
设计哲学：物理主机访问权 = Admin 权。
运营者 SSH 到主机后才能使用 CLI 命令；CLI 直接读写 KmsDb，不走 HTTP。
```

现有先例：`bin/api_key.rs`（generate / list / revoke）已遵循此模式。

新增 CLI 二进制 `bin/kms_admin.rs`，子命令：

```
kms-admin rotate-jwt-secret [--force]   # 立即轮换 kms_secret（紧急时用）
kms-admin jwt-secret-status             # 列出各 kid 版本 + 状态 + 年龄 + retireAt
kms-admin list-agent-keys [--account <key_id>]  # 查询 agent 密钥列表
kms-admin revoke-agent-key <key_id>     # 强制吊销（运营者紧急措施）
```

**通过 x-api-key 对外暴露的权限清单（完整）**：

| 端点 | 权限说明 |
|------|---------|
| `POST /kms/CreateKey` | 创建密钥（需 WebAuthn） |
| `POST /kms/Sign` | 签名（需 WebAuthn） |
| `POST /kms/DescribeKey` | 查询密钥信息 |
| `POST /kms/ListKeys` | 列出密钥 |
| `POST /kms/DeriveAddress` | 派生地址 |
| `POST /kms/GetPublicKey` | 获取公钥 |
| `POST /kms/DeleteKey` | 删除密钥（需 WebAuthn） |
| `POST /kms/ChangePasskey` | 更换 passkey |
| `POST /kms/create-agent-key` | 创建 Agent 密钥（需 WebAuthn）**【新增】** |
| `POST /kms/sign-agent` | Agent 签名（Bearer JWT）**【新增】** |
| `POST /kms/refresh-agent-credential` | 刷新凭证（需 WebAuthn）**【新增】** |
| `POST /kms/revoke-agent-credential` | 吊销凭证（需 WebAuthn）**【新增】** |
| `GET /health` | 健康检查（公开） |
| `GET /stats` | 统计（公开） |

> 结论：对外 HTTP 权限全部有 WebAuthn 或 JWT 二次鉴权保护，无"裸 admin"端点。
> 后续如需追加 admin 能力，优先走 CLI，不往 HTTP 追加。

### 1.4 kms_secret 生命周期

```
初始化：KMS 首次启动时，检查 TEE 封存存储 → 无则 TRNG 生成 v1，kid=v1，标记 current
自动轮换（tokio 后台任务，每 24h 触发）：
  if current_kid.age >= 30d:
    1. TEE 内 TRNG 生成新密钥 → kid=v(n+1)，标记 current（此后新 JWT 用它签）
    2. 旧 kid=v(n) → 标记 verify-only，retireAt = now + 7d
    3. retireAt 到期 → 删除旧密钥
任意时刻最多 2 把并存（1 current + 1 verify-only）
```

| 参数 | 值 |
|------|-----|
| JWT TTL | 3 天 |
| kms_secret 签发期 | 30 天 |
| verify-only 重叠窗口 | 7 天（≥ JWT TTL，留余量） |
| 单把 kms_secret 总寿命 | 37 天后删除 |

紧急轮换：SSH 登主机 → `kms-admin rotate-jwt-secret --force`（可选 `--revoke-old` 立即废弃旧密钥，所有在用 JWT 失效）。

---

## 2. 新增 HTTP 端点

### 2.1 `POST /kms/create-agent-key`

鉴权：Bearer x-api-key（已有）+ body 内 WebAuthn assertion（同现有 sign 端点）

```json
// Request
{
  "humanKeyId": "uuid-of-human-account",
  "label": "my-trading-bot",            // optional, 备注
  "passkey_assertion": { ... }          // WebAuthn
}

// Response
{
  "keyId": "agent-uuid",
  "agentAddress": "0x...",             // 链上用此地址 grantAgentSession
  "derivationPath": "m/44'/60'/0'/1/0",
  "agentCredential": "eyJ...",         // JWT，单次可见，丢失需 refresh
  "expiresAt": 1234567890
}
```

### 2.2 `POST /kms/sign-agent`

鉴权：`Authorization: Bearer <agentCredential>`（JWT HS256，TEE 内验）

```json
// Request
{
  "keyId": "agent-uuid",
  "payload": "0x1234...abcd",          // 裸 userOpHash，32 bytes hex
  "algorithm": "secp256k1"
}

// Response
{
  "keyId": "agent-uuid",
  "agentAddress": "0x...",
  "signature": "0x<R(32)><S(32)><V(1)>"  // 65 bytes，V=27/28
}

// 错误码
// 401: JWT 无效/过期/已吊销
// 403: keyId 与 JWT payload 不匹配
// 404: keyId 不存在
// 422: payload 格式错误（非 32 bytes hex）
// 429: 速率限制
```

Rate limit：per-keyId 滑动窗口（P0），默认 100 次/分钟，可配置。

### 2.3 `POST /kms/refresh-agent-credential`

鉴权：当前 JWT（验证有效）+ WebAuthn assertion（人类重认证）

```json
// Request
{
  "keyId": "agent-uuid",
  "passkey_assertion": { ... }

}
// Response: 同 create-agent-key（新 JWT，EOA 不变）
```

### 2.4 `POST /kms/revoke-agent-credential`

鉴权：WebAuthn assertion

```json
// Request
{ "keyId": "agent-uuid", "passkey_assertion": { ... } }
// Response: { "success": true, "revokedAt": 1234567890 }
// 效果：此后任何该 keyId 的 JWT 均返回 401
```

---

## 3. DB Schema 扩展

### 3.1 `agent_keys` 表

```sql
CREATE TABLE IF NOT EXISTS agent_keys (
  key_id          TEXT PRIMARY KEY,        -- UUID v4
  human_account   TEXT NOT NULL,           -- 关联人类账户 key_id
  agent_address   TEXT NOT NULL,           -- secp256k1 以太坊地址
  derivation_path TEXT NOT NULL,           -- m/44'/60'/0'/1/<index>
  label           TEXT NOT NULL DEFAULT '',
  credential_hash TEXT,                    -- SHA-256(JWT)，NULL = 已吊销
  created_at      INTEGER NOT NULL,
  revoked         INTEGER NOT NULL DEFAULT 0,
  revoked_at      INTEGER,
  FOREIGN KEY (human_account) REFERENCES wallets(key_id)
);
CREATE INDEX idx_agent_keys_human ON agent_keys(human_account);
```

### 3.2 `jwt_secret_meta` 表（元信息，密钥本体在 TEE）

```sql
CREATE TABLE IF NOT EXISTS jwt_secret_meta (
  kid         TEXT PRIMARY KEY,         -- "v1", "v2", ...
  status      TEXT NOT NULL,            -- "current" | "verify-only" | "retired"
  created_at  INTEGER NOT NULL,
  retire_at   INTEGER                   -- verify-only 到期删除时间（NULL = current）
);
```

---

## 4. TA 新增命令

在 `kms/ta/src/main.rs` 新增 5 个命令（同时提供 mock TEE 路径）：

| 命令 | 输入 | 输出 | 说明 |
|------|------|------|------|
| `CMD_CREATE_AGENT_KEY` | `{humanKeyId, agentIndex}` | `{agentAddress}` | HD 派生 secp256k1，封存 |
| `CMD_SIGN_AGENT_USEROP` | `{keyId, userOpHash_hex}` | `{r, s, v}` | EIP-191 包裹 + 签名，v=27/28 |
| `CMD_JWT_ISSUE` | `{keyId, humanAccount, agentId, ttl_secs}` | `{jwt, exp}` | HS256 签发，kms_secret 在 TEE |
| `CMD_JWT_VERIFY` | `{jwt}` | `{valid, payload}` | 验证签名 + exp，按 kid 选密钥 |
| `CMD_JWT_ROTATE_SECRET` | `{force: bool}` | `{new_kid}` | 生成新 kid，旧转 verify-only |

---

## 5. 现有资产复用

| 组件 | 文件 | 复用方式 |
|------|------|---------|
| secp256k1 BIP32 派生 | `kms/ta/src/bip32_secp.rs` | 直接复用，传入 agentIndex |
| WebAuthn 鉴权 | `kms/host/src/webauthn.rs` | 复用 `pre_verify_passkey` |
| Rate limiting | `kms/host/src/rate_limit.rs` | 扩展为 per-keyId 粒度 |
| SQLite DB 层 | `kms/host/src/db.rs` | 加新表 + migration |
| HTTP 路由 (warp) | `kms/host/src/api_server.rs` | 新增 4 个 handler |
| CLI 模式 | `kms/host/src/bin/api_key.rs` | 参照，新建 `bin/kms_admin.rs` |

---

## 6. 实现顺序

```
Step 1: proto 扩展         kms/proto/src/in_out.rs  — 新 Input/Output 结构体
Step 2: DB migration       kms/host/src/db.rs        — agent_keys + jwt_secret_meta
Step 3: TA 新命令          kms/ta/src/main.rs        — 5 个命令 + mock 路径
Step 4: JWT 凭证系统       kms/host/src/agent_jwt.rs — issue/verify/revoke
Step 5: kms_secret 轮换    kms/host/src/api_server.rs — tokio background task
Step 6: HTTP 端点          kms/host/src/api_server.rs — 4 个 handler
Step 7: Rate limit 扩展    kms/host/src/rate_limit.rs — per-keyId
Step 8: CLI admin          kms/host/src/bin/kms_admin.rs — 4 个子命令
Step 9: 测试脚本           docs/quick-curl-test-commands.md — agent 场景覆盖
```

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| TA 内 HMAC-SHA256 GP API 兼容性 | OP-TEE 提供 `TEE_MACComputeFinal`；先验证已有 TA 是否已用 GP Crypto API（WebAuthn 验证时用过则肯定可用） |
| V 归一化遗漏 (0/1 → 27/28) | `sign_agent_userop` 函数内强制 `if v < 27 { v += 27 }`，单元测试覆盖两种 parity |
| mock TEE 路径遗漏 | 每个 TA 命令在 `#[cfg(feature = "mock-tee")]` 分支同步实现 |
| kms_secret 状态不一致 | JWT meta 存 DB，密钥本体存 TEE；两者以 kid 为 key 对齐；rotate 操作原子：先 TEE 写入成功再更新 DB |
| credential_hash 碰撞 | SHA-256，碰撞概率可忽略；validate 时做全等比较不做前缀匹配 |

---

## 8. 不在本次范围

- `POST /kms/hwrng`（可选，§4 of spec）— defer
- `POST /kms/derive-agent-key`（HD 选项 §5）— 已内置于 create-agent-key，单独端点 defer
- 链上 `grantAgentSession` 调用 — SDK 侧负责，KMS 只提供 agentAddress
- 审计日志持久化 — P1，Sprint 2
