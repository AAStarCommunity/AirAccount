# eth_wallet 项目深度分析与KMS改造方案

*创建时间: 2025-09-30*

## 📋 目录

1. [项目概述](#项目概述)
2. [构建流程分析](#构建流程分析)
3. [xargo问题诊断与解决](#xargo问题诊断与解决)
4. [运行与测试流程](#运行与测试流程)
5. [KMS改造架构设计](#kms改造架构设计)
6. [实施计划](#实施计划)

---

## 项目概述

### 核心功能

eth_wallet是一个**Ethereum钱包TA参考实现**,提供:

| 功能 | 说明 | 安全性 |
|------|------|--------|
| **Key Generation** | TEE内生成随机种子 | 🔒 硬件RNG |
| **Key Derivation** | BIP32/BIP44密钥派生 | 🔒 TEE内计算 |
| **Key Persistency** | 密钥安全存储 | ⚠️ REE FS加密 |
| **Transaction Signing** | EIP-155签名 | 🔒 私钥不出TEE |
| **Key Erase** | 密钥删除 | 🔒 安全擦除 |

### 项目结构

```
eth_wallet/
├── Makefile                 # 顶层构建入口
├── README.md               # 项目文档
├── uuid.txt                # TA UUID: be2dc9a0-02b4-4b33-ba21-9964dbdf1573
├── proto/                  # 共享协议定义
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Command枚举
│       └── in_out.rs       # 输入输出结构
├── ta/                     # Trusted Application (Secure World)
│   ├── Makefile            # ⚠️ 使用xargo (问题所在)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # TA入口点
│       └── wallet.rs       # 钱包核心逻辑
└── host/                   # Client Application (Normal World)
    ├── Makefile
    ├── Cargo.toml
    └── src/
        ├── main.rs         # CA入口点
        └── cli.rs          # 命令行接口
```

---

## 构建流程分析

### 顶层Makefile (项目根目录)

```makefile
# eth_wallet/Makefile
BUILDER ?= cargo          # ✅ 可配置构建器
FEATURES ?=

all: host ta

host:
	make -C host TARGET=$(TARGET_HOST) \
		CROSS_COMPILE=$(CROSS_COMPILE_HOST)

ta:
	make -C ta TARGET=$(TARGET_TA) \
		CROSS_COMPILE=$(CROSS_COMPILE_TA) \
		BUILDER=$(BUILDER) \              # ✅ 传递BUILDER变量
		FEATURES="$(FEATURES)"
```

**分析**: 顶层Makefile支持配置BUILDER,但TA的Makefile忽略了它。

### TA Makefile (ta/Makefile)

```makefile
# eth_wallet/ta/Makefile - ⚠️ 问题代码
clippy:
	@cargo fmt
	@xargo clippy --target $(TARGET) -- -D warnings    # ❌ 硬编码xargo

ta: clippy
	@xargo build --target $(TARGET) --release          # ❌ 硬编码xargo
```

**问题**: 直接硬编码使用`xargo`,忽略了`BUILDER`变量。

### Hello World TA Makefile (正确示例)

```makefile
# hello_world-rs/ta/Makefile - ✅ 正确实现
BUILDER ?= cargo          # ✅ 可配置

clippy:
	@cargo fmt
	@RUSTFLAGS="$(RUSTFLAGS)" $(BUILDER) clippy ...    # ✅ 使用变量

ta: clippy
	@RUSTFLAGS="$(RUSTFLAGS)" $(BUILDER) build ...     # ✅ 使用变量
```

---

## xargo问题诊断与解决

### 问题诊断

#### 1. **xargo是什么?**

- **历史**: xargo是cargo的封装,用于no_std环境交叉编译
- **现状**: **已废弃** (2020年后不再维护)
- **替代**: 现代cargo原生支持no_std + 自定义target

#### 2. **为什么eth_wallet使用xargo?**

- eth_wallet是**旧项目** (2020年左右创建)
- 当时cargo对no_std支持不完善
- 现在cargo已原生支持这些功能

#### 3. **为什么hello_world能用cargo?**

| 特性 | hello_world | eth_wallet |
|------|-------------|------------|
| **创建时间** | 2024+ | 2020 |
| **构建工具** | cargo | xargo |
| **Rust版本** | 1.80.0-nightly | 旧版本 |
| **Target支持** | 原生 | 需要xargo |

### 解决方案

#### 方案1: 修改TA Makefile使用cargo (推荐)

```makefile
# 修改 eth_wallet/ta/Makefile

UUID ?= $(shell cat "../uuid.txt")
TARGET ?= aarch64-unknown-linux-gnu
CROSS_COMPILE ?= aarch64-linux-gnu-
OBJCOPY := $(CROSS_COMPILE)objcopy
LINKER_CFG := target.$(TARGET).linker=\"$(CROSS_COMPILE)gcc\"
RUSTFLAGS := -C panic=abort

TA_SIGN_KEY ?= $(TA_DEV_KIT_DIR)/keys/default_ta.pem
SIGN := $(TA_DEV_KIT_DIR)/scripts/sign_encrypt.py
OUT_DIR := $(CURDIR)/target/$(TARGET)/release

BUILDER ?= cargo          # ✅ 添加BUILDER变量
FEATURES ?=

all: clippy ta strip sign

clippy:
	@cargo fmt
	@RUSTFLAGS="$(RUSTFLAGS)" $(BUILDER) clippy --target $(TARGET) $(FEATURES) -- -D warnings

ta: clippy
	@RUSTFLAGS="$(RUSTFLAGS)" $(BUILDER) build --target $(TARGET) --release $(FEATURES) --config $(LINKER_CFG)

strip: ta
	@$(OBJCOPY) --strip-unneeded $(OUT_DIR)/ta $(OUT_DIR)/stripped_ta

sign: strip
	@$(SIGN) --uuid $(UUID) --key $(TA_SIGN_KEY) --in $(OUT_DIR)/stripped_ta --out $(OUT_DIR)/$(UUID).ta
	@echo "SIGN =>  ${UUID}"

clean:
	@cargo clean
```

**变更说明**:
1. 添加 `BUILDER ?= cargo` (默认cargo)
2. 将所有 `xargo` 替换为 `$(BUILDER)`
3. 添加 `RUSTFLAGS` 支持 `-C panic=abort`
4. 修正 `OUT_DIR` 路径 (去掉`_TA`后缀)

#### 方案2: 安装xargo (不推荐)

```bash
# 容器内安装xargo
cargo install xargo
rustup component add rust-src
```

**问题**: xargo已废弃,不建议在新项目中使用。

---

## 运行与测试流程

### 标准运行流程

#### 1. 构建

```bash
# 在容器内
cd /root/teaclave_sdk_src/projects/web3/eth_wallet
make
```

#### 2. 部署到QEMU

```bash
# 复制TA到optee_armtz
cp ta/target/aarch64-unknown-linux-gnu/release/be2dc9a0-*.ta \
   /opt/teaclave/shared/ta/

# 复制Host应用
cp host/target/aarch64-unknown-linux-gnu/release/eth_wallet-rs \
   /opt/teaclave/shared/host/
```

#### 3. 在Guest VM中运行

```bash
# Terminal 2 (Guest VM)
mkdir -p shared && mount -t 9p -o trans=virtio host shared
cd shared
cp be2dc9a0-*.ta /lib/optee_armtz/
./eth_wallet-rs
```

### CLI命令测试

```bash
# 1. 创建钱包
./eth_wallet-rs create-wallet
# 输出: Wallet ID: aa5798a1-3c89-4708-b316-712aea4f59e2

# 2. 派生地址
./eth_wallet-rs derive-address -w aa5798a1-3c89-4708-b316-712aea4f59e2
# 输出: Address: 0x7ca2b64a29bbf7a77bf8a3187ab09f50413826ea

# 3. 签名交易
./eth_wallet-rs sign-transaction \
  -t 0xc0ffee254729296a45a3885639AC7E10F9d54979 \
  -v 100 \
  -w aa5798a1-3c89-4708-b316-712aea4f59e2
# 输出: Signature: "f86380843b9aca00..."

# 4. 删除钱包
./eth_wallet-rs remove-wallet -w aa5798a1-3c89-4708-b316-712aea4f59e2
```

---

## KMS改造架构设计

### 改造目标

将eth_wallet改造为**通用KMS服务**,支持:
1. 多种密钥算法 (secp256k1, Ed25519, RSA)
2. AWS KMS兼容API
3. HTTP/gRPC API服务
4. 企业级密钥管理

### 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                    KMS Service                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │           Host Application (CA)                   │  │
│  │  ┌────────────────────────────────────────────┐  │  │
│  │  │      HTTP/gRPC API Server (api.rs)         │  │  │
│  │  │  - CreateKey, Sign, GetPublicKey           │  │  │
│  │  │  - ListKeys, DescribeKey                   │  │  │
│  │  │  - ScheduleKeyDeletion                     │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────┐  │  │
│  │  │      TEEC Client (ta_client.rs)            │  │  │
│  │  │  - 连接TA                                   │  │  │
│  │  │  - 序列化/反序列化请求                      │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────┐  │  │
│  │  │      Metadata Store (metadata.rs)          │  │  │
│  │  │  - 密钥元数据 (名称、创建时间、状态)        │  │  │
│  │  │  - 审计日志                                 │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────┘  │
│                         ⬍ TEEC API                      │
│  ┌──────────────────────────────────────────────────┐  │
│  │       Trusted Application (TA) - Secure World    │  │
│  │  ┌────────────────────────────────────────────┐  │  │
│  │  │      Command Handler (main.rs)             │  │  │
│  │  │  - CreateKey, DeriveKey, Sign              │  │  │
│  │  │  - GetPublicKey, DeleteKey                 │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────┐  │  │
│  │  │      Crypto Engine (crypto.rs)             │  │  │
│  │  │  - secp256k1 (以太坊、比特币)               │  │  │
│  │  │  - Ed25519 (Solana、Polkadot)              │  │  │
│  │  │  - RSA (传统系统)                           │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  │  ┌────────────────────────────────────────────┐  │  │
│  │  │      Secure Storage (storage.rs)           │  │  │
│  │  │  - 密钥材料存储                             │  │  │
│  │  │  - OP-TEE Secure Storage                   │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 模块设计

#### 1. TA模块 (基于eth_wallet/ta)

```
kms-ta/
├── Cargo.toml
└── src/
    ├── main.rs          # TA入口,命令分发
    ├── crypto.rs        # 密码学引擎
    │   ├── secp256k1.rs  # 现有eth_wallet功能
    │   ├── ed25519.rs    # 新增
    │   └── rsa.rs        # 新增
    ├── storage.rs       # 安全存储 (复用eth_wallet的SecureDB)
    └── key.rs           # 密钥抽象
```

#### 2. Host模块 (扩展eth_wallet/host)

```
kms-host/
├── Cargo.toml
└── src/
    ├── main.rs          # 主程序入口
    ├── api.rs           # 🆕 HTTP/gRPC API服务器
    ├── ta_client.rs     # TEEC客户端
    ├── metadata.rs      # 🆕 元数据管理
    ├── cli.rs           # CLI接口 (保留)
    └── types.rs         # 类型定义
```

#### 3. Proto模块 (扩展eth_wallet/proto)

```
kms-proto/
├── Cargo.toml
└── src/
    ├── lib.rs           # Command枚举
    └── in_out.rs        # 输入输出结构
        ├── CreateKeyInput/Output
        ├── SignInput/Output
        ├── GetPublicKeyInput/Output
        └── ...
```

### API设计

#### AWS KMS兼容API

```rust
// kms-host/src/api.rs

use axum::{Router, Json};
use serde::{Serialize, Deserialize};

#[derive(Deserialize)]
struct CreateKeyRequest {
    #[serde(rename = "KeyUsage")]
    key_usage: String,          // "SIGN_VERIFY"

    #[serde(rename = "KeySpec")]
    key_spec: String,           // "ECC_SECG_P256K1", "ECC_ED25519"
}

#[derive(Serialize)]
struct CreateKeyResponse {
    #[serde(rename = "KeyMetadata")]
    key_metadata: KeyMetadata,
}

pub async fn create_key(
    Json(req): Json<CreateKeyRequest>
) -> Json<CreateKeyResponse> {
    // 1. 调用TA创建密钥
    let key_id = ta_client::create_key(&req.key_spec).await?;

    // 2. 保存元数据
    metadata::save(key_id, KeyMetadata {
        key_id,
        key_spec: req.key_spec,
        key_usage: req.key_usage,
        created_at: Utc::now(),
    })?;

    // 3. 返回响应
    Ok(Json(CreateKeyResponse { key_metadata }))
}
```

---

## 实施计划

### Phase 1: 修复xargo问题 (1天)

**任务**:
1. ✅ 修改 `eth_wallet/ta/Makefile`
2. ✅ 测试构建
3. ✅ 验证QEMU运行

**验证标准**:
```bash
cd projects/web3/eth_wallet
make
# 应该成功构建,无xargo错误
```

### Phase 2: 复制并重命名项目 (1天)

**任务**:
1. 复制eth_wallet到 `kms/`
2. 重命名模块: `eth_wallet-rs` → `kms-service`
3. 生成新UUID
4. 更新所有引用

**目录结构**:
```
kms/
├── kms-ta/          # 从eth_wallet/ta复制
├── kms-host/        # 从eth_wallet/host复制
└── kms-proto/       # 从eth_wallet/proto复制
```

### Phase 3: 添加API服务 (2-3天)

**任务**:
1. 在kms-host中添加 `api.rs`
2. 集成Axum HTTP服务器
3. 实现AWS KMS兼容端点
4. 添加元数据管理

**代码示例**:
```rust
// kms-host/src/api.rs
use axum::{Router, routing::post};

pub fn create_router() -> Router {
    Router::new()
        .route("/", post(handle_kms_request))
        .route("/health", get(health_check))
}

async fn handle_kms_request(
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, Error> {
    // 解析X-Amz-Target头
    let target = headers.get("X-Amz-Target")
        .ok_or(Error::MissingTarget)?;

    match target.as_str() {
        "TrentService.CreateKey" => create_key(body).await,
        "TrentService.Sign" => sign(body).await,
        _ => Err(Error::UnknownOperation),
    }
}
```

### Phase 4: 测试与优化 (2-3天)

**任务**:
1. 单元测试
2. 集成测试
3. QEMU环境测试
4. 性能优化

---

## 关键问题讨论

### Q1: 为什么不直接修改eth_wallet?

**A**:
- ✅ **保持原始参考实现**: eth_wallet是Apache项目的参考实现
- ✅ **独立演进**: KMS需求与eth_wallet不同
- ✅ **简化维护**: 避免上游更新冲突

### Q2: xargo vs cargo的技术细节?

**A**:
```
xargo (废弃):
- 为no_std环境编译std组件
- 需要rust-src
- 额外的构建步骤

cargo (现代):
- 原生支持自定义target
- 内置-Z build-std
- 更好的工具链集成
```

### Q3: API服务放在Host还是独立进程?

**A**:
**推荐**: 放在Host应用内

```
方案1 (推荐): Host内API
  kms-host → [TEEC] → kms-ta
  ✅ 简单部署
  ✅ 减少通信开销
  ⚠️ 单点故障

方案2: 独立API服务
  api-server → kms-host → [TEEC] → kms-ta
  ✅ 可扩展
  ✅ 负载均衡
  ❌ 增加复杂度
```

### Q4: 元数据存储在哪里?

**A**:
```
阶段1 (开发):
  - 内存HashMap
  - 简单快速

阶段2 (测试):
  - SQLite本地数据库
  - 持久化

阶段3 (生产):
  - PostgreSQL
  - 高可用
```

---

## 下一步行动

### 立即执行

1. **修复xargo问题**
   ```bash
   # 修改ta/Makefile
   vim third_party/teaclave-trustzone-sdk/projects/web3/eth_wallet/ta/Makefile
   # 应用上述修复方案
   ```

2. **验证构建**
   ```bash
   docker exec teaclave_dev_env bash -l -c \
     "cd /root/teaclave_sdk_src/projects/web3/eth_wallet && make"
   ```

3. **测试运行**
   ```bash
   # 在Guest VM中测试所有命令
   ./eth_wallet-rs create-wallet
   ./eth_wallet-rs derive-address -w <wallet-id>
   ./eth_wallet-rs sign-transaction ...
   ```

### 后续计划

- [ ] 完成Phase 1: xargo修复
- [ ] 完成Phase 2: 项目复制和重命名
- [ ] 完成Phase 3: API服务集成
- [ ] 完成Phase 4: 测试和优化

---

*最后更新: 2025-09-30*