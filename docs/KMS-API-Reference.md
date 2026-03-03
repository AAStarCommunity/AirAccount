# KMS API Reference (STM32 DK2)

> Last updated: 2026-03-03 +07

Base URL: `https://kms1.aastar.io` (production) / `http://192.168.7.2:3000` (local DK2)

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [API Overview Table](#2-api-overview-table)
3. [Authentication](#3-authentication)
4. [WebAuthn Ceremony Flow](#4-webauthn-ceremony-flow)
5. [HTTP API — Wallet Management](#5-http-api--wallet-management)
6. [HTTP API — WebAuthn Ceremony](#6-http-api--webauthn-ceremony)
7. [HTTP API — Signing](#7-http-api--signing)
8. [HTTP API — Query & Status](#8-http-api--query--status)
9. [TA Commands (Proto Layer)](#9-ta-commands-proto-layer)
10. [CLI Tools](#10-cli-tools)
11. [CA-Side Database](#11-ca-side-database)
12. [Shared Data Structures](#12-shared-data-structures)
13. [Performance Notes](#13-performance-notes)
14. [Environment Variables](#14-environment-variables)
15. [Quick Start (三步走)](#15-quick-start-三步走)
16. [Troubleshooting](#16-troubleshooting)

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Normal World (CA)                            │
│                                                                     │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────────┐    │
│  │  api-key CLI │   │ export_key   │   │   kms-api-server     │    │
│  │  (管理APIKey) │   │  (导出私钥)   │   │   (HTTP :3000)       │    │
│  └──────┬───────┘   └──────┬───────┘   │                      │    │
│         │                  │           │  ┌─────────────────┐  │    │
│         ▼                  │           │  │  WebAuthn 验证   │  │    │
│  ┌──────────────┐          │           │  │  CA 预验证 P-256 │  │    │
│  │  SQLite DB   │◄─────────┼───────────┤  │  API Key 中间件  │  │    │
│  │  (kms.db)    │          │           │  └─────────────────┘  │    │
│  └──────────────┘          │           │          │             │    │
│                            │           │  ┌───────▼─────────┐  │    │
│                            │           │  │   TeeHandle     │  │    │
│                            ▼           │  │ (持久TEE session)│  │    │
│                      ┌─────────┐       │  └───────┬─────────┘  │    │
│                      │TaClient │       └──────────┼────────────┘    │
│                      └────┬────┘                  │                 │
├───────────────────────────┼───────────────────────┼─────────────────┤
│                    Secure World (TA)              │                 │
│  ┌────────────────────────▼───────────────────────▼──────────────┐  │
│  │  TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd               │  │
│  │                                                               │  │
│  │  ┌───────────┐  ┌──────────────┐  ┌────────────────────────┐ │  │
│  │  │ LRU Cache │  │ PBKDF2 Seed  │  │  OP-TEE Secure Storage │ │  │
│  │  │ (200 slot)│  │  Cache       │  │  (HUK encrypted)       │ │  │
│  │  └───────────┘  └──────────────┘  └────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

三层调用链：**Client → CA HTTP API → TA Command (proto)**

- **CA (kms-api-server)**: HTTP 服务器，负责请求验证、DB 持久化、WebAuthn 仪式、API Key 鉴权、PassKey CA 预验证
- **TA**: OP-TEE Trusted Application，负责钱包创建/签名/密钥派生，运行在 ARM TrustZone 安全世界
- **Proto**: CA↔TA 通信协议层，bincode 序列化，定义 Command 枚举和 Input/Output 结构

---

## 2. API Overview Table

### HTTP API (CA 层暴露)

| # | Method | Path | x-amz-target | API Key | PassKey | 场景 |
|---|--------|------|-------------|---------|---------|------|
| 1 | POST | `/CreateKey` | `TrentService.CreateKey` | Yes | 创建时绑定 | 创建新 HD 钱包 |
| 2 | POST | `/DescribeKey` | `TrentService.DescribeKey` | Yes | No | 查询钱包元数据 |
| 3 | POST | `/ListKeys` | `TrentService.ListKeys` | Yes | No | 列出所有钱包 |
| 4 | POST | `/DeriveAddress` | `TrentService.DeriveAddress` | Yes | Yes | 派生以太坊地址 |
| 5 | POST | `/Sign` | `TrentService.Sign` | Yes | Yes | 签名交易/消息 |
| 6 | POST | `/SignHash` | `TrentService.SignHash` | Yes | Yes | 签名 32 字节 hash |
| 7 | POST | `/GetPublicKey` | `TrentService.GetPublicKey` | Yes | No | 获取公钥 |
| 8 | POST | `/DeleteKey` | `TrentService.ScheduleKeyDeletion` | Yes | Yes | 删除钱包 |
| 9 | POST | `/ChangePasskey` | — | Yes | Yes (当前) | 更换 PassKey 公钥 |
| 10 | POST | `/BeginRegistration` | — | Yes | No | WebAuthn 注册仪式 Step 1 |
| 11 | POST | `/CompleteRegistration` | — | Yes | No | WebAuthn 注册仪式 Step 2 |
| 12 | POST | `/BeginAuthentication` | — | Yes | No | WebAuthn 认证仪式 |
| 13 | GET | `/KeyStatus` | — | No | No | 轮询钱包派生状态 |
| 14 | GET | `/QueueStatus` | — | No | No | 查询 TEE 队列深度 |
| 15 | GET | `/health` | — | No | No | 健康检查 |

### TA Commands (Proto 层)

| # | Command | u32 | 场景 | PassKey |
|---|---------|-----|------|---------|
| 1 | `CreateWallet` | 0 | 创建钱包 + 绑定 passkey pubkey | 创建时绑定 |
| 2 | `RemoveWallet` | 1 | 从安全存储删除钱包 | Optional |
| 3 | `DeriveAddress` | 2 | BIP32 派生地址 | Optional |
| 4 | `SignTransaction` | 3 | 签名 ETH 交易 (RLP) | Optional |
| 5 | `SignMessage` | 4 | 签名任意消息 | Optional |
| 6 | `SignHash` | 5 | 签名 32 字节 hash | Optional |
| 7 | `DeriveAddressAuto` | 6 | 自动派生 m/44'/60'/0'/0/0 | No |
| 8 | `ExportPrivateKey` | 7 | 导出私钥明文（仅调试） | Optional |
| 9 | `VerifyPasskey` | 8 | 验证 P-256 ECDSA 签名 | 直接验证 |
| 10 | `WarmupCache` | 9 | 预热 LRU 缓存 | No |
| 11 | `RegisterPasskeyTa` | 10 | 更换 passkey 公钥 | Yes (当前) |

### CLI 工具

| 工具 | 二进制名 | 场景 | 需要 TEE |
|------|---------|------|---------|
| API 服务器 | `kms-api-server` | 运行 HTTP 服务 | Yes |
| API Key 管理 | `api-key` | 生成/列出/吊销 API Key | No |
| 导出私钥 | `export_key` | 调试用，导出钱包私钥明文 | Yes |
| 旧版 CLI | `kms` | create-wallet / derive-address / sign-transaction | Yes |

---

## 3. Authentication

### 3.1 API Key

API Key 用于保护 KMS API 不被未授权的客户端调用。

**启用条件**（满足任一即启用）：
- SQLite DB 中存在至少一条 api_key 记录
- 环境变量 `KMS_API_KEY` 已设置

**验证逻辑**：
1. 从请求 header `x-api-key` 提取 key
2. 先检查 `KMS_API_KEY` 环境变量（legacy 兼容）
3. 再检查 DB `api_keys` 表
4. 任一匹配即放行

**当前默认行为**：如果 DB 无 key 且无环境变量 → 所有请求放行（无鉴权模式）。

> **关闭无鉴权模式**：只需运行 `api-key generate` 生成至少一个 key，API Key 验证自动启用。无需修改代码或重启服务（DB 实时查询）。但 `kms-api-server` 启动时的日志消息需要重启才会更新。

**API Key 管理**（通过 `api-key` CLI 工具）：

```bash
# 生成新 key（stdout 只输出 key 本身，方便管道）
api-key generate --label "sdk-backend"
# 输出: kms_4319f3510b244097b65980ee4f824cdd

# 列出所有 key（中间打码）
api-key list
# KEY                                      LABEL                CREATED
# kms_431...4cdd                           sdk-backend          2026-03-02T15:31:13+00:00

# 吊销
api-key revoke kms_4319f3510b244097b65980ee4f824cdd
```

**DB 存储** (`api_keys` 表)：

| 字段 | 类型 | 说明 |
|------|------|------|
| `api_key` | TEXT PK | 完整 key (`kms_<uuid-hex>`) |
| `label` | TEXT | 标签（如 "sdk-backend"） |
| `created_at` | TEXT | 创建时间 (RFC3339) |

**DB 方法**：
| 方法 | 说明 |
|------|------|
| `generate_api_key(label)` | 生成 `kms_<uuid-hex>` 格式 key，存 DB，返回明文 |
| `validate_api_key(key)` | 校验 key 是否存在 |
| `list_api_keys()` | 返回 `Vec<(key, label, created_at)>` |
| `revoke_api_key(key)` | 删除 key，返回是否成功 |
| `has_api_keys()` | DB 中是否有任何 key |

### 3.2 PassKey (P-256 ECDSA)

所有敏感操作（Sign/SignHash/DeriveAddress/DeleteKey/ChangePasskey）需要 PassKey 认证。支持两种格式：

**方式 A — Legacy hex 格式**（直接传 hex 编码的 assertion）：
```json
{
  "Passkey": {
    "AuthenticatorData": "0x<hex>",
    "ClientDataHash": "0x<sha256-of-clientDataJSON-32bytes>",
    "Signature": "0x<DER-or-64byte-r||s>"
  }
}
```

**方式 B — WebAuthn ceremony 格式**（推荐，通过 BeginAuthentication 获取 challenge）：
```json
{
  "WebAuthn": {
    "ChallengeId": "<from-BeginAuthentication>",
    "Credential": {
      "id": "<base64url>",
      "rawId": "<base64url>",
      "response": {
        "clientDataJSON": "<base64url>",
        "authenticatorData": "<base64url>",
        "signature": "<base64url-DER>"
      },
      "type": "public-key"
    }
  }
}
```

**双重验证**：
1. **CA 预验证**：CA 用 P-256 ECDSA 验证签名，拦截无效请求（不浪费 TA 队列）
2. **TA 验证**：TA 在安全世界再次验证（最终信任源）

---

## 4. WebAuthn Ceremony Flow

### 4.1 Registration (注册 — 创建钱包 + 绑定 PassKey)

兼容 [SimpleWebAuthn](https://simplewebauthn.dev/) 标准流程。

```
 Browser/SDK                    KMS API (CA)                         TA
     │                              │                                 │
     │  POST /BeginRegistration     │                                 │
     │  { Description, UserName }   │                                 │
     │─────────────────────────────>│                                 │
     │                              │  generate challenge             │
     │                              │  store challenge in DB          │
     │  { ChallengeId, Options }    │                                 │
     │<─────────────────────────────│                                 │
     │                              │                                 │
     │  navigator.credentials       │                                 │
     │    .create(Options)          │                                 │
     │  ══════════════════>         │                                 │
     │  [Authenticator prompt]      │                                 │
     │  <══════════════════         │                                 │
     │  credential response         │                                 │
     │                              │                                 │
     │  POST /CompleteRegistration  │                                 │
     │  { ChallengeId, Credential } │                                 │
     │─────────────────────────────>│                                 │
     │                              │  consume challenge              │
     │                              │  verify clientDataJSON          │
     │                              │    (type, challenge, origin)    │
     │                              │  decode attestationObject (CBOR)│
     │                              │  verify rpIdHash                │
     │                              │  extract P-256 pubkey from COSE │
     │                              │                                 │
     │                              │  create_wallet(pubkey)          │
     │                              │────────────────────────────────>│
     │                              │  wallet_id                      │
     │                              │<────────────────────────────────│
     │                              │                                 │
     │                              │  insert wallet to DB            │
     │                              │  spawn background derive_addr   │
     │                              │────────────────────────────────>│
     │                              │                                 │
     │  { KeyId, CredentialId,      │                                 │
     │    Status: "deriving" }      │                                 │
     │<─────────────────────────────│                                 │
     │                              │                                 │
     │  GET /KeyStatus?KeyId=xxx    │         (60-75s later)          │
     │─────────────────────────────>│  { Status:"ready", Address }    │
     │<─────────────────────────────│                                 │
```

### 4.2 Authentication (认证 — 签名前获取 challenge)

```
 Browser/SDK                    KMS API (CA)                         TA
     │                              │                                 │
     │  POST /BeginAuthentication   │                                 │
     │  { KeyId or Address }        │                                 │
     │─────────────────────────────>│                                 │
     │                              │  generate challenge             │
     │                              │  store challenge in DB          │
     │                              │  (bound to key_id)              │
     │  { ChallengeId, Options }    │                                 │
     │<─────────────────────────────│                                 │
     │                              │                                 │
     │  navigator.credentials       │                                 │
     │    .get(Options)             │                                 │
     │  ══════════════════>         │                                 │
     │  [Authenticator prompt]      │                                 │
     │  <══════════════════         │                                 │
     │  assertion response          │                                 │
     │                              │                                 │
     │  POST /SignHash              │                                 │
     │  { KeyId, Hash,              │                                 │
     │    WebAuthn: {               │                                 │
     │      ChallengeId,            │                                 │
     │      Credential              │                                 │
     │    }                         │                                 │
     │  }                           │                                 │
     │─────────────────────────────>│                                 │
     │                              │  consume challenge              │
     │                              │  verify assertion               │
     │                              │    (challenge, origin, rpId,    │
     │                              │     signCount, ECDSA P-256)     │
     │                              │  update sign_count in DB        │
     │                              │  convert to proto assertion     │
     │                              │                                 │
     │                              │  sign_hash(assertion)           │
     │                              │────────────────────────────────>│
     │                              │  signature                      │
     │                              │<────────────────────────────────│
     │  { Signature }               │                                 │
     │<─────────────────────────────│                                 │
```

### 4.3 Legacy Flow (不用 WebAuthn ceremony)

使用 `CreateKey` + `PasskeyPublicKey` 直接绑定公钥，签名时用 `Passkey` 字段传 hex assertion。适用于已有 Relying Party 的场景。

---

## 5. HTTP API — Wallet Management

### 5.1 POST /CreateKey

创建新 HD 钱包。立即返回 KeyId，地址派生在后台异步执行。

**Headers**: `x-amz-target: TrentService.CreateKey`, `x-api-key` (if enabled)

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `KeyId` | string | No | 自定义 KeyId（默认自动生成 UUID） |
| `Description` | string | Yes | 钱包描述 |
| `KeyUsage` | string | Yes | 固定 `"SIGN_VERIFY"` |
| `KeySpec` | string | Yes | 固定 `"ECC_SECG_P256K1"` |
| `Origin` | string | Yes | 固定 `"EXTERNAL_KMS"` |
| `PasskeyPublicKey` | string | Yes | P-256 uncompressed hex (`0x04||x||y`, 65 bytes) |

**Response**:
```json
{
  "KeyMetadata": {
    "KeyId": "c45a955b-2e50-41bf-8331-3a6de70b27e6",
    "Arn": "arn:aws:kms:region:account:key/c45a955b-...",
    "CreationDate": "2026-03-02T15:00:00Z",
    "Enabled": true,
    "Description": "my-wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "EXTERNAL_KMS",
    "PasskeyPublicKey": "0x04..."
  },
  "Mnemonic": "[MNEMONIC_IN_SECURE_WORLD]"
}
```

**场景**: SDK 后端直接提供 P-256 公钥创建钱包（不走 WebAuthn ceremony）。创建后轮询 `/KeyStatus` 直到 `"ready"`。

**TA 命令**: `CreateWallet` → `DeriveAddressAuto` (background)

---

### 5.2 POST /DescribeKey

查询钱包详细元数据。

**Headers**: `x-amz-target: TrentService.DescribeKey`, `x-api-key`

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `KeyId` | string | Yes | 钱包 UUID |

**Response**:
```json
{
  "KeyMetadata": {
    "KeyId": "c45a955b-...",
    "Address": "0x51671e4d...",
    "PublicKey": "0x03799170...",
    "DerivationPath": "m/44'/60'/0'/0/0",
    "PasskeyPublicKey": "0x04...",
    "Arn": "arn:aws:kms:...",
    "CreationDate": "2026-03-02T15:00:00Z",
    "Enabled": true,
    "Description": "my-wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "EXTERNAL_KMS"
  }
}
```

**场景**: 查看钱包当前状态、地址、公钥等信息。数据来自 CA SQLite DB。

---

### 5.3 POST /ListKeys

列出所有钱包。

**Headers**: `x-amz-target: TrentService.ListKeys`, `x-api-key`

**Request**: `{}` (body 可为空 JSON)

**Response**:
```json
{
  "Keys": [
    { "KeyId": "c45a955b-...", "KeyArn": "arn:aws:kms:..." },
    { "KeyId": "d78b1234-...", "KeyArn": "arn:aws:kms:..." }
  ]
}
```

**场景**: 管理面板展示钱包列表。数据来自 CA SQLite DB（持久化，重启不丢失）。

---

### 5.4 POST /DeleteKey

删除钱包（TA 安全存储 + CA DB 同时删除）。

**Headers**: `x-amz-target: TrentService.ScheduleKeyDeletion`, `x-api-key`

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `KeyId` | string | Yes | 钱包 UUID |
| `PendingWindowInDays` | int | No | 预留天数（当前忽略，立即删除） |
| `Passkey` | object | 二选一 | Legacy hex assertion |
| `WebAuthn` | object | 二选一 | WebAuthn ceremony assertion |

**Response**:
```json
{
  "KeyId": "c45a955b-...",
  "DeletionDate": "2026-03-09T15:00:00Z"
}
```

**场景**: 永久销毁钱包。TA 侧删除 entropy + seed cache，CA 侧删除 wallet + address_index（CASCADE）。

**TA 命令**: `RemoveWallet`

---

### 5.5 POST /ChangePasskey

更换钱包的 PassKey 公钥。需要当前 PassKey 认证。

**Headers**: `x-api-key`（无 x-amz-target）

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `KeyId` | string | Yes | 钱包 UUID |
| `PasskeyPublicKey` | string | Yes | 新 P-256 公钥 hex (`0x04...`) |
| `Passkey` | object | 二选一 | 当前 passkey legacy assertion |
| `WebAuthn` | object | 二选一 | 当前 passkey WebAuthn assertion |

**Response**:
```json
{
  "KeyId": "c45a955b-...",
  "Changed": true
}
```

**场景**: 用户更换认证设备（如换手机），需要用旧设备授权新设备公钥。

**TA 命令**: `RegisterPasskeyTa`

---

## 6. HTTP API — WebAuthn Ceremony

### 6.1 POST /BeginRegistration

WebAuthn 注册仪式第一步。返回 `PublicKeyCredentialCreationOptions`，前端传给 `navigator.credentials.create()`。

**Headers**: `x-api-key`

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `Description` | string | No | 钱包描述（默认 `""`) |
| `UserName` | string | No | WebAuthn 用户名（默认 `"wallet-user"`) |
| `UserDisplayName` | string | No | 显示名（默认 `"AirAccount Wallet"`) |
| `KeyUsage` | string | No | 默认 `"SIGN_VERIFY"` |
| `KeySpec` | string | No | 默认 `"ECC_SECG_P256K1"` |
| `Origin` | string | No | 默认 `"EXTERNAL_KMS"` |

**Response**:
```json
{
  "ChallengeId": "a1b2c3d4-...",
  "Options": {
    "rp": { "name": "AirAccount KMS", "id": "aastar.io" },
    "user": {
      "id": "<base64url>",
      "name": "wallet-user",
      "displayName": "AirAccount Wallet"
    },
    "challenge": "<base64url-32bytes>",
    "pubKeyCredParams": [{ "type": "public-key", "alg": -7 }],
    "timeout": 300000,
    "attestation": "none",
    "excludeCredentials": [],
    "authenticatorSelection": {
      "residentKey": "preferred",
      "userVerification": "required"
    }
  }
}
```

**注意**:
- `alg: -7` = ES256 (P-256 ECDSA with SHA-256)
- `ChallengeId` 有效期 300 秒
- 前端必须保存 `ChallengeId`，在 CompleteRegistration 中回传

---

### 6.2 POST /CompleteRegistration

WebAuthn 注册仪式第二步。提交认证器响应，CA 验证后在 TA 创建钱包。

**Headers**: `x-api-key`

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `ChallengeId` | string | Yes | 来自 BeginRegistration |
| `Credential` | object | Yes | `navigator.credentials.create()` 返回值 |
| `Description` | string | No | 覆盖 BeginRegistration 时的描述 |

`Credential` 结构（标准 `RegistrationResponseJSON`）：
```json
{
  "id": "<base64url-credential-id>",
  "rawId": "<base64url>",
  "response": {
    "clientDataJSON": "<base64url>",
    "attestationObject": "<base64url>",
    "transports": ["internal", "hybrid"]
  },
  "type": "public-key",
  "authenticatorAttachment": "platform"
}
```

**Response**:
```json
{
  "KeyId": "c45a955b-2e50-41bf-8331-3a6de70b27e6",
  "CredentialId": "<base64url>",
  "Status": "deriving"
}
```

**CA 验证步骤**：
1. 消费 challenge（一次性）
2. 解码 `clientDataJSON`，验证 `type="webauthn.create"`, `challenge` 匹配, `origin` 匹配
3. 解码 `attestationObject` (CBOR)，提取 `authData`
4. 验证 `rpIdHash = SHA-256(rp_id)`
5. 检查 User Presence (UP) 和 Attested Credential (AT) 标志位
6. 从 COSE key 提取 P-256 公钥 (x, y → 0x04||x||y)
7. 用提取的公钥调用 TA `CreateWallet`

**TA 命令**: `CreateWallet` → `DeriveAddressAuto` (background)

---

### 6.3 POST /BeginAuthentication

WebAuthn 认证仪式。返回 `PublicKeyCredentialRequestOptions`，前端传给 `navigator.credentials.get()`。

**Headers**: `x-api-key`

**Request**（二选一）：
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `KeyId` | string | 二选一 | 钱包 UUID |
| `Address` | string | 二选一 | 以太坊地址（自动查找 KeyId） |

**Response**:
```json
{
  "ChallengeId": "e5f6g7h8-...",
  "Options": {
    "challenge": "<base64url-32bytes>",
    "timeout": 300000,
    "rpId": "aastar.io",
    "allowCredentials": [
      {
        "id": "<base64url-credential-id>",
        "type": "public-key",
        "transports": ["internal", "hybrid"]
      }
    ],
    "userVerification": "required"
  }
}
```

**后续**: 前端拿到认证器 response 后，在 Sign/SignHash/DeleteKey 等请求的 `WebAuthn` 字段中传入 `ChallengeId` + `Credential`。

---

## 7. HTTP API — Signing

### 7.1 POST /Sign

签名以太坊交易或任意消息。

**Headers**: `x-amz-target: TrentService.Sign`, `x-api-key`

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `Address` | string | 模式1 | 以太坊地址（优先级最高，自动查 KeyId + Path） |
| `KeyId` | string | 模式2 | 钱包 UUID |
| `DerivationPath` | string | 模式2 | HD 路径 (如 `m/44'/60'/0'/0/0`) |
| `Transaction` | object | 二选一 | ETH 交易对象 |
| `Message` | string | 二选一 | 消息 (hex `0x...` 或 base64 或 UTF-8) |
| `SigningAlgorithm` | string | No | 固定 `"ECDSA_SHA_256"` |
| `Passkey` | object | 认证 | Legacy hex assertion |
| `WebAuthn` | object | 认证 | WebAuthn ceremony assertion |

**Transaction 结构**:
```json
{
  "chainId": 11155111,
  "nonce": 0,
  "to": "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
  "value": "0xde0b6b3a7640000",
  "gasPrice": "0x4a817c800",
  "gas": 21000,
  "data": ""
}
```

**Response**:
```json
{
  "Signature": "3045022100...",
  "TransactionHash": "[TX_HASH_OR_MESSAGE_HASH]"
}
```

**三种调用模式**:
1. **Address 模式**: 只传 `Address`，CA 从 DB 自动查 `KeyId` + `DerivationPath`
2. **KeyId + Path 模式**: 指定 `KeyId` 和 `DerivationPath`
3. **Transaction vs Message**: 传 `Transaction` 走 RLP 编码签名，传 `Message` 走 EIP-191 消息签名

**TA 命令**: `SignTransaction` 或 `SignMessage`

---

### 7.2 POST /SignHash

直接签名 32 字节 hash（不做额外 hashing）。

**Headers**: `x-amz-target: TrentService.SignHash`, `x-api-key`

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `Address` | string | 模式1 | 以太坊地址 |
| `KeyId` | string | 模式2 | 钱包 UUID |
| `DerivationPath` | string | No | HD 路径（可选，默认用钱包已有路径） |
| `Hash` | string | Yes | 32 字节 hex (`0x...` 或无前缀) |
| `Passkey` | object | 认证 | Legacy hex assertion |
| `WebAuthn` | object | 认证 | WebAuthn ceremony assertion |

**Response**:
```json
{
  "Signature": "3045022100..."
}
```

**三种地址解析方式**:
1. `Address` → 从 DB 查 KeyId + Path
2. `KeyId` + `DerivationPath` → 直接使用
3. `KeyId` only → 使用钱包默认 Path

**TA 命令**: `SignHash`

---

### 7.3 POST /GetPublicKey

获取钱包公钥。

**Headers**: `x-amz-target: TrentService.GetPublicKey`, `x-api-key`

**Request**:
| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `KeyId` | string | Yes | 钱包 UUID |

**Response**:
```json
{
  "KeyId": "c45a955b-...",
  "PublicKey": "0x03799170...",
  "KeyUsage": "SIGN_VERIFY",
  "KeySpec": "ECC_SECG_P256K1"
}
```

**场景**: 客户端需要公钥做本地验签或地址生成。数据来自 CA DB。

---

## 8. HTTP API — Query & Status

### 8.1 GET /KeyStatus?KeyId=\<uuid\>

轮询钱包地址派生进度。

**无需认证**。

| Status | 含义 |
|--------|------|
| `creating` | 钱包已创建，派生未开始 |
| `deriving` | BIP32 派生进行中 (~60-75s 首次) |
| `ready` | 地址和公钥就绪 |
| `error` | 派生失败（查看 `Error` 字段） |

**Response** (ready):
```json
{
  "KeyId": "c45a955b-...",
  "Status": "ready",
  "Address": "0x51671e4d896d718208b549d05bb6f6d8a7f5b89e",
  "PublicKey": "0x03799170bf8863a004acd475640b1588af391d9d79ff3d4d1c5a5b32669f64498b",
  "DerivationPath": "m/44'/60'/0'/0/0"
}
```

**场景**: 创建钱包后前端轮询，显示 loading → ready 过渡。

---

### 8.2 GET /QueueStatus

查询 TEE 操作队列深度。

**Response**:
```json
{
  "queue_depth": 1,
  "estimated_wait_seconds": 1
}
```

TEE 是单线程的，所有操作排队执行。`estimated_wait_seconds = queue_depth × 1s`（热路径估算）。

---

### 8.3 GET /health

健康检查。

**Response**:
```json
{
  "status": "healthy",
  "service": "kms-api",
  "version": "0.1.0",
  "ta_mode": "real",
  "endpoints": {
    "POST": ["/CreateKey", "/DeleteKey", "/DescribeKey", "/ListKeys",
             "/DeriveAddress", "/Sign", "/SignHash", "/ChangePasskey",
             "/BeginRegistration", "/CompleteRegistration", "/BeginAuthentication"],
    "GET": ["/health", "/KeyStatus?KeyId=xxx", "/QueueStatus"]
  }
}
```

---

## 9. TA Commands (Proto Layer)

Proto 层定义了 CA↔TA 之间的通信协议。所有 Input/Output 使用 `bincode` 序列化。

### 9.1 CreateWallet (0)

| 方向 | 结构 |
|------|------|
| Input | `passkey_pubkey: Vec<u8>` — P-256 uncompressed 65 bytes |
| Output | `wallet_id: Uuid`, `mnemonic: String` |

在 TA 安全存储中生成 BIP-39 助记词和 entropy，绑定 passkey 公钥。

### 9.2 RemoveWallet (1)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `passkey_assertion: Option<PasskeyAssertion>` |
| Output | (空) |

从安全存储永久删除钱包 entropy + seed cache。

### 9.3 DeriveAddress (2)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `hd_path: String`, `passkey_assertion: Option<PasskeyAssertion>` |
| Output | `address: [u8; 20]`, `public_key: Vec<u8>` |

BIP-32 HD 密钥派生 → secp256k1 公钥 → Keccak-256 → ETH 地址。

### 9.4 SignTransaction (3)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `hd_path: String`, `transaction: EthTransaction`, `passkey_assertion: Option<PasskeyAssertion>` |
| Output | `signature: Vec<u8>` |

RLP 编码交易 → Keccak-256 → secp256k1 ECDSA 签名 → RLP 编码 signed tx。

`EthTransaction`: `{ chain_id: u64, nonce: u128, to: Option<[u8;20]>, value: u128, gas_price: u128, gas: u128, data: Vec<u8> }`

### 9.5 SignMessage (4)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `hd_path: String`, `message: Vec<u8>`, `passkey_assertion: Option<PasskeyAssertion>` |
| Output | `signature: Vec<u8>` |

EIP-191 prefix (`\x19Ethereum Signed Message:\n{len}`) + message → Keccak-256 → ECDSA 签名。

### 9.6 SignHash (5)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `hd_path: String`, `hash: [u8; 32]`, `passkey_assertion: Option<PasskeyAssertion>` |
| Output | `signature: Vec<u8>` |

直接对 32 字节 hash 做 secp256k1 ECDSA 签名，不做额外 hashing。

### 9.7 DeriveAddressAuto (6)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid` |
| Output | `wallet_id: Uuid`, `address: [u8; 20]`, `public_key: Vec<u8>`, `derivation_path: String` |

内部命令，自动使用 `m/44'/60'/0'/0/0` 派生。由 CreateKey 后台自动触发，不需要 passkey。

### 9.8 ExportPrivateKey (7)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `derivation_path: String`, `passkey_assertion: Option<PasskeyAssertion>` |
| Output | `private_key: Vec<u8>` — 32 bytes |

**仅调试用**。导出 HD 派生的私钥明文。生产环境应禁用。

### 9.9 VerifyPasskey (8)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `public_key: Vec<u8>`, `authenticator_data: Vec<u8>`, `client_data_hash: [u8;32]`, `signature_r: [u8;32]`, `signature_s: [u8;32]` |
| Output | `valid: bool` |

TA 内验证 P-256 ECDSA 签名。独立于 passkey_assertion 流程，用于 standalone 验签测试。

### 9.10 WarmupCache (9)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid` |
| Output | `cached: bool`, `cache_size: u32` |

预热 TA LRU 缓存（加载 wallet entropy + PBKDF2 seed 到内存）。已从公开 API 移除，仅 TA 内部保留。

### 9.11 RegisterPasskeyTa (10)

| 方向 | 结构 |
|------|------|
| Input | `wallet_id: Uuid`, `passkey_pubkey: Vec<u8>`, `passkey_assertion: Option<PasskeyAssertion>` |
| Output | `registered: bool` |

在 TA 安全存储中更换 passkey 公钥。需要当前 passkey assertion 授权（防止未授权更换）。

### PasskeyAssertion (共享结构)

```
authenticator_data: Vec<u8>     — WebAuthn authenticatorData
client_data_hash:   [u8; 32]    — SHA-256(clientDataJSON)
signature_r:        [u8; 32]    — ECDSA P-256 r
signature_s:        [u8; 32]    — ECDSA P-256 s
```

---

## 10. CLI Tools

所有 CLI 工具运行在 DK2 板上（SSH `root@192.168.7.2`）。

### 10.1 kms-api-server

主 HTTP API 服务器。

```bash
# 启动
kms-api-server
# 或指定 DB 路径
KMS_DB_PATH=/data/kms/kms.db kms-api-server
```

监听 `0.0.0.0:3000`。通过 systemd 管理：
```bash
systemctl start kms
systemctl stop kms
systemctl status kms
```

### 10.2 api-key

API Key 生命周期管理。**不需要 TEE 环境**，只操作 SQLite DB。

```bash
# 生成新 key（stdout 输出完整 key，stderr 输出提示）
api-key generate --label "sdk-backend"
# stdout: kms_4319f3510b244097b65980ee4f824cdd
# stderr: API key generated. Label: "sdk-backend"
#         Store this key securely — it cannot be retrieved later.

# 列出所有 key（中间打码显示）
api-key list
# KEY                                      LABEL                CREATED
# ────────────────────────────────────────────────────────────────────────
# kms_431...4cdd                           sdk-backend          2026-03-02T15:31:13+00:00
# 1 key(s) total.

# 吊销指定 key
api-key revoke kms_4319f3510b244097b65980ee4f824cdd
# API key revoked.

# 帮助
api-key help
```

**DB 路径**：`KMS_DB_PATH` 环境变量 > `/data/kms/kms.db` > `./kms.db`

### 10.3 export_key

调试工具，导出钱包私钥明文。**需要 TEE 环境**。

```bash
export_key <wallet-uuid> <derivation-path>
export_key c45a955b-2e50-41bf-8331-3a6de70b27e6 "m/44'/60'/0'/0/0"
# 输出: 0x<64-hex-chars>
```

**安全警告**: 仅限开发/调试环境使用。生产部署时应从构建中排除此二进制。

### 10.4 kms (旧版 CLI)

早期 CLI 工具，现已被 API 服务器替代。仍可用于直接 TA 交互。

```bash
# 创建钱包
kms create-wallet

# 派生地址
kms derive-address -w <wallet-uuid> -d "m/44'/60'/0'/0/0"

# 签名交易
kms sign-transaction -w <wallet-uuid> -t <to-address> -v <value> --chain-id 11155111

# 运行测试
kms test
```

---

## 11. CA-Side Database

SQLite WAL 模式，存储所有 CA 侧持久化数据。

**路径优先级**: `KMS_DB_PATH` > `/data/kms/kms.db` > `./kms.db`

### Schema

```sql
-- 钱包表
CREATE TABLE wallets (
    key_id          TEXT PRIMARY KEY,    -- UUID
    address         TEXT,                -- 0x... ETH 地址 (派生后填入)
    public_key      TEXT,                -- 0x... secp256k1 公钥
    derivation_path TEXT,                -- m/44'/60'/0'/0/0
    description     TEXT NOT NULL,
    key_usage       TEXT NOT NULL,       -- SIGN_VERIFY
    key_spec        TEXT NOT NULL,       -- ECC_SECG_P256K1
    origin          TEXT NOT NULL,       -- EXTERNAL_KMS
    passkey_pubkey  TEXT,                -- 0x04... P-256 公钥 hex
    credential_id   TEXT,                -- base64url WebAuthn credential ID
    sign_count      INTEGER NOT NULL,    -- WebAuthn signCount
    status          TEXT NOT NULL,       -- creating | deriving | ready | error
    error_msg       TEXT,
    created_at      TEXT NOT NULL        -- RFC3339
);

-- 地址索引 (Address → KeyId 快速查找)
CREATE TABLE address_index (
    address         TEXT PRIMARY KEY,    -- 0x... ETH 地址
    key_id          TEXT NOT NULL,       -- FK → wallets
    derivation_path TEXT NOT NULL,
    public_key      TEXT,
    FOREIGN KEY (key_id) REFERENCES wallets(key_id) ON DELETE CASCADE
);

-- WebAuthn Challenge 存储 (一次性消费)
CREATE TABLE challenges (
    id              TEXT PRIMARY KEY,    -- UUID
    challenge       BLOB NOT NULL,       -- 32 bytes random
    key_id          TEXT,                -- bound to wallet (authentication)
    purpose         TEXT NOT NULL,       -- registration | authentication
    rp_id           TEXT NOT NULL,
    created_at      INTEGER NOT NULL,    -- unix timestamp
    expires_at      INTEGER NOT NULL     -- unix timestamp (default: +300s)
);

-- API Key 表
CREATE TABLE api_keys (
    api_key         TEXT PRIMARY KEY,    -- kms_<uuid-hex>
    label           TEXT NOT NULL,
    created_at      TEXT NOT NULL        -- RFC3339
);

-- 索引
CREATE INDEX idx_address_key ON address_index(key_id);
CREATE INDEX idx_challenge_expire ON challenges(expires_at);
CREATE INDEX idx_wallet_credential ON wallets(credential_id);
```

### 数据恢复

如果 CA DB 丢失，钱包数据可从 TA 安全存储恢复：
- TA 中存有 entropy、passkey_pubkey
- 重新调用 `DeriveAddressAuto` 可恢复 address/public_key/derivation_path
- WebAuthn credential_id 和 sign_count 无法从 TA 恢复（需前端重新注册）

---

## 12. Shared Data Structures

### KeyMetadata (响应中的钱包元数据)

```json
{
  "KeyId": "c45a955b-...",
  "Address": "0x51671e4d...",
  "PublicKey": "0x03799170...",
  "DerivationPath": "m/44'/60'/0'/0/0",
  "Arn": "arn:aws:kms:region:account:key/c45a955b-...",
  "CreationDate": "2026-03-02T15:00:00Z",
  "Enabled": true,
  "Description": "my-wallet",
  "KeyUsage": "SIGN_VERIFY",
  "KeySpec": "ECC_SECG_P256K1",
  "Origin": "EXTERNAL_KMS",
  "PasskeyPublicKey": "0x04..."
}
```

### PasskeyAssertion (Legacy hex 格式)

```json
{
  "AuthenticatorData": "0x<hex>",
  "ClientDataHash": "0x<32-bytes-sha256-hex>",
  "Signature": "0x<DER-or-64byte-r||s-hex>"
}
```

Signature 支持两种格式：
- DER 编码（可变长度，通常 70-72 bytes）
- Raw r||s（固定 64 bytes）

### WebAuthnAssertion (Ceremony 格式)

```json
{
  "ChallengeId": "<from-BeginAuthentication>",
  "Credential": { /* AuthenticationResponseJSON */ }
}
```

### AuthenticationResponseJSON (标准 WebAuthn)

```json
{
  "id": "<base64url-credential-id>",
  "rawId": "<base64url>",
  "response": {
    "clientDataJSON": "<base64url>",
    "authenticatorData": "<base64url>",
    "signature": "<base64url-DER>",
    "userHandle": "<base64url-optional>"
  },
  "type": "public-key",
  "clientExtensionResults": {}
}
```

### RegistrationResponseJSON (标准 WebAuthn)

```json
{
  "id": "<base64url-credential-id>",
  "rawId": "<base64url>",
  "response": {
    "clientDataJSON": "<base64url>",
    "attestationObject": "<base64url>",
    "transports": ["internal", "hybrid"]
  },
  "type": "public-key",
  "authenticatorAttachment": "platform",
  "clientExtensionResults": {}
}
```

---

## 13. Performance Notes

STM32MP157F-DK2, Cortex-A7 @ 650MHz, measured 2026-03-02

### 13.1 Full API Benchmark (p256-m optimized, 2026-03-03)

| Operation | Time | Notes |
|-----------|------|-------|
| GET /health | **<5ms** | HTTP round-trip only |
| GET /QueueStatus | **~23ms** | CA-only |
| POST /CreateKey | **~7.2s** | PBKDF2 + wallet + secure storage write |
| Background derivation (poll KeyStatus) | **~90s** | HD key derivation, PBKDF2 if cold |
| POST /DescribeKey | **~23ms** | CA-only (SQLite DB) |
| POST /ListKeys | **~22ms** | CA-only |
| POST /SignHash (hot) | **~960ms** | p256-m verify (~100ms) + secp256k1 sign |
| POST /Sign (message) | **~1.0s** | EIP-191 message sign |
| POST /Sign (transaction) | **~1.7s** | TX RLP encode + sign |
| POST /GetPublicKey | **~23ms** | CA-only (no TA call needed) |
| POST /DeriveAddress | **~920ms** | TA HD derivation + P-256 verify |
| POST /DeleteKey | **~4.1s** | Secure storage delete |

### 13.2 Performance Breakdown

| Component | Cost | Notes |
|-----------|------|-------|
| P-256 ECDSA verify (p256-m, TA) | **~100ms** | Was ~2s with OP-TEE native or Rust p256 |
| P-256 ECDSA verify (p256 crate, CA) | **~20ms** | CA pre-verify |
| secp256k1 sign (cached seed) | **~800ms** | |
| Transaction RLP encode + sign | **~1.5s** | |
| PBKDF2-HMAC-SHA512 (2048 rounds) | **~55s** | |
| Secure storage write | **~0.5-1s** | |
| HTTP round-trip (CA-only) | **~20-25ms** | |

### 13.3 Historical Comparison

| Metric | No PassKey (v0.1) | OP-TEE native P-256 | p256-m (current) |
|--------|-------------------|---------------------|------------------|
| SignHash (hot) | **0.83~1.12s** | **~3.0s** | **~960ms** |
| Sign (message) | ~1s | ~3.1s | **~1.0s** |
| CreateKey | ~3.5s | ~6.1s | **~7.2s** |
| P-256 verify | N/A | ~2s | **~100ms** |

p256-m (C library, ~3KB code) 将 P-256 ECDSA 验证从 ~2s 降至 ~100ms，使得添加 PassKey 后 SignHash 总耗时与无 PassKey 版本基本持平。

### 13.4 Notes

- **PBKDF2 瓶颈**: 2048 rounds SHA-512 在 32-bit ARM 耗时 ~55s。Seed 缓存后跳过。
- **TEE 单线程**: 所有 TA 操作排队执行。检查 `/QueueStatus` 估算等待时间。
- **Address cache**: Sign by Address 依赖 CA 内存缓存，新建 wallet 需等 background derivation 完成或手动 rebuild-cache。
- **CA-side pre-verification**: 新版 CA 代码在 CA 层做 P-256 预验证，无效 assertion 不进 TA 队列（节省 ~3s TA 占用时间）。

---

## 14. Environment Variables

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `KMS_DB_PATH` | `/data/kms/kms.db` 或 `./kms.db` | SQLite DB 路径 |
| `KMS_API_KEY` | (无) | Legacy API key（兼容旧配置） |
| `KMS_RP_ID` | `aastar.io` | WebAuthn Relying Party ID |
| `KMS_RP_NAME` | `AirAccount KMS` | WebAuthn Relying Party 显示名 |
| `KMS_ORIGIN` | `https://{KMS_RP_ID}` | WebAuthn Expected Origin |
| `RUST_LOG` | (无) | 日志级别 (e.g. `info`, `debug`) |

---

## 15. Quick Start (三步走)

### Step 1: 编译

```bash
cd kms/scripts
./build.sh          # 编译 TA + CA
./build.sh ta       # 仅编译 TA
./build.sh ca       # 仅编译 CA
```

### Step 2: 部署

```bash
./deploy.sh         # 部署到 DK2 (192.168.7.2)
./deploy.sh ta      # 仅部署 TA
./deploy.sh ca      # 仅部署 CA
```

### Step 3: 测试

```bash
./run-all-tests.sh  # 单元测试 + API 测试 + 性能基准
```

单独运行：
```bash
# 仅单元测试（本地，无需 DK2）
cd kms/host && cargo test --no-default-features --lib

# 仅 API 测试
cd kms/test && ./run-api-tests.sh

# 仅性能测试（5 轮）
cd kms/test && ./perf-test.sh 192.168.7.2:3000 5
```

---

## 16. Troubleshooting

### 日志查看

```bash
# CA 服务日志
ssh root@192.168.7.2 "journalctl -u kms -f"

# TA 日志 (如果内核支持)
ssh root@192.168.7.2 "cat /proc/tee_log"

# 实时查看 KMS 服务状态
ssh root@192.168.7.2 "systemctl status kms"
```

### 常见错误排查

| 错误现象 | 可能原因 | 排查方法 |
|---------|---------|---------|
| `TEE_ERROR_TARGET_DEAD (0xffff3024)` | TA panic（内存越界、unwrap 失败等） | 查看 `/proc/tee_log`（如可用），或重启 `tee-supplicant` |
| `Connection refused :3000` | kms-api-server 未运行 | `systemctl status kms`，查看 journalctl |
| `API key required` | 已启用 API Key 但请求未携带 | 添加 `-H "x-api-key: kms_..."` header |
| `PassKey verification failed (CA)` | passkey assertion 签名无效 | 检查 P-256 签名是否正确（auth_data \|\| cdh 的 SHA-256） |
| `Challenge not found or expired` | WebAuthn challenge 已过期（>300s） | 重新调用 BeginAuthentication |
| CreateKey 后 KeyStatus 一直 `deriving` | PBKDF2 执行中（首次约 90s） | 等待或检查 TA 是否正常（journalctl） |
| `file in wrong format` (编译) | CA 使用了错误的交叉编译器 | 设置 `CC=/tmp/arm-ca-gcc` |
| `cannot represent machine 'aarch64'` (编译) | `TARGET_TA` 未设置 | `export TARGET_TA=arm-unknown-optee` |

### 手动 API 调试

```bash
# 健康检查
curl http://192.168.7.2:3000/health

# 查看队列深度
curl http://192.168.7.2:3000/QueueStatus

# 创建钱包 (需要真实 P-256 公钥)
curl -X POST http://192.168.7.2:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d '{"Description":"test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS","PasskeyPublicKey":"0x04..."}'

# 生成测试用 P-256 keypair + assertion
cd kms/test
python3 p256_helper.py gen          # 生成 keypair
python3 p256_helper.py assertion <pem>  # 生成 assertion
```

### 重置环境

```bash
# 停止服务
ssh root@192.168.7.2 "systemctl stop kms"

# 删除 CA 数据库 (钱包可从 TA 恢复)
ssh root@192.168.7.2 "rm -f /root/shared/kms.db"

# 重启
ssh root@192.168.7.2 "systemctl start kms"
```
