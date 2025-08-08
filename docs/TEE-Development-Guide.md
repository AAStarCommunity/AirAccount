# AirAccount TEE开发指南 - 基于QEMU的完整实践

## 概述

本指南详细说明了如何使用Apache Teaclave TrustZone SDK在本地QEMU环境中开发TEE（可信执行环境）应用，特别是针对AirAccount项目的私钥管理和签名服务。

## 目录

- [1. 环境准备](#1-环境准备)
- [2. 项目架构设计](#2-项目架构设计)
- [3. QEMU环境搭建](#3-qemu环境搭建)
- [4. TA开发实践](#4-ta开发实践)
- [5. CA开发实践](#5-ca开发实践)
- [6. 调试与测试](#6-调试与测试)
- [7. 三步开发策略评估](#7-三步开发策略评估)

## 1. 环境准备

### 1.1 系统要求

- **操作系统**: Ubuntu 20.04/22.04 LTS (推荐) 或 macOS
- **硬盘空间**: 最少20GB可用空间
- **内存**: 至少8GB RAM（推荐16GB+）
- **网络**: 稳定的互联网连接

### 1.2 依赖工具安装

#### Ubuntu环境
```bash
# 更新系统
sudo apt update && sudo apt upgrade -y

# 安装基础工具
sudo apt install -y \
    build-essential \
    git \
    curl \
    python3 \
    python3-pip \
    uuid-dev \
    libssl-dev \
    libffi-dev \
    libglib2.0-dev \
    libpixman-1-dev \
    ninja-build \
    pkg-config \
    gcc-multilib \
    qemu-system-arm \
    qemu-user-static
```

#### macOS环境
```bash
# 安装Homebrew（如未安装）
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 安装依赖
brew install automake coreutils curl gmp gnutls libtool libusb make wget qemu
```

### 1.3 Rust工具链设置

```bash
# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 添加必要的target
rustup target add aarch64-unknown-linux-gnu
rustup target add aarch64-unknown-optee-trustzone
rustup target add armv7-unknown-linux-gnueabihf

# 安装cargo-make（可选，用于复杂构建流程）
cargo install cargo-make
```

## 2. 项目架构设计

### 2.1 整体架构

AirAccount项目采用三层架构设计：

```
┌─────────────────────────────────────┐
│           Core Logic Layer          │ <- 90%可复用业务逻辑
├─────────────────────────────────────┤
│          TEE Adapter Layer          │ <- 平台特定适配
├─────────────────────────────────────┤
│           TA Entry Point            │ <- TEE实现入口点
└─────────────────────────────────────┘
```

### 2.2 组件划分

```
packages/
├── core-logic/           # 硬件无关的核心逻辑
│   ├── src/
│   │   ├── crypto.rs     # 加密算法封装
│   │   ├── wallet.rs     # 钱包核心逻辑
│   │   └── types.rs      # 共享类型定义
│   └── Cargo.toml
├── ta-arm-trustzone/     # ARM TrustZone TA实现
│   ├── src/
│   │   ├── main.rs       # TA入口点
│   │   └── secure_ops.rs # 安全操作
│   ├── ta.rs             # TA配置
│   └── Cargo.toml
├── client-tauri/         # Tauri客户端应用
│   ├── src/
│   │   ├── main.rs       # CA主程序
│   │   └── tee_client.rs # TEE客户端接口
│   └── Cargo.toml
└── shared/               # 共享接口定义
    ├── src/
    │   ├── protocol.rs   # 通信协议
    │   └── commands.rs   # 命令定义
    └── Cargo.toml
```

### 2.3 TA架构设计（基于eth_wallet）

#### 核心功能模块

```rust
// packages/ta-arm-trustzone/src/main.rs
use core_logic::{WalletManager, CryptoProvider};

pub struct AirAccountTA {
    wallet_manager: WalletManager,
    crypto: CryptoProvider,
}

impl AirAccountTA {
    // 核心命令处理
    fn handle_create_wallet(&mut self) -> Result<WalletInfo>;
    fn handle_sign_transaction(&mut self, tx_hash: &[u8]) -> Result<Signature>;
    fn handle_get_public_key(&self, derivation_path: &str) -> Result<PublicKey>;
    fn handle_verify_fingerprint(&self, fp_data: &[u8]) -> Result<bool>;
}
```

#### 安全存储设计

```rust
// 私钥存储策略
pub struct SecureStorage {
    // 使用OP-TEE安全存储API
    storage_id: StorageID,
}

impl SecureStorage {
    fn store_master_key(&self, key: &[u8]) -> Result<()>;
    fn retrieve_master_key(&self) -> Result<Vec<u8>>;
    fn derive_key(&self, path: &str) -> Result<PrivateKey>;
}
```

### 2.4 CA架构设计

#### HTTP服务接口

```rust
// packages/client-tauri/src/main.rs
use axum::{Router, routing::post};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SignRequest {
    pub transaction_hash: String,
    pub derivation_path: String,
    pub fingerprint_data: Vec<u8>,
}

async fn sign_transaction(
    Json(req): Json<SignRequest>
) -> Result<Json<SignResponse>, AppError> {
    // 1. 验证指纹
    // 2. 调用TEE进行签名
    // 3. 返回签名结果
}
```

## 3. QEMU环境搭建

### 3.1 获取Teaclave TrustZone SDK

```bash
# 克隆SDK及子模块
git clone --recursive https://github.com/apache/incubator-teaclave-trustzone-sdk.git
cd incubator-teaclave-trustzone-sdk

# 如果忘记使用--recursive
git submodule update --init --recursive
```

### 3.2 构建工具链

```bash
# 下载并构建交叉编译工具链
make toolchains

# 这个过程需要20-60分钟，取决于网络和硬件
```

### 3.3 构建QEMU TEE环境

```bash
# 构建支持ARMv8-A的QEMU环境
make optee-qemuv8

# 首次构建可能需要1-2小时
```

### 3.4 验证环境

```bash
# 启动QEMU环境
make run-qemuv8

# 你会看到两个窗口：
# 1. Normal World (Linux) - 用于运行CA
# 2. Secure World (OP-TEE) - 显示TA日志

# 在Normal World终端中运行测试
xtest -l 3
```

### 3.5 Docker方式（可选）

```bash
# 使用提供的Dockerfile
docker build -f docker/Dockerfile.qemu -t teaclave-dev .
docker run -it --privileged teaclave-dev
```

## 4. TA开发实践

### 4.1 创建AirAccount TA项目

```bash
# 在SDK根目录下创建项目
mkdir -p projects/airaccount
cd projects/airaccount

# 创建目录结构
mkdir -p {ta,host,shared,proto}/src
```

### 4.2 定义共享接口

```rust
// shared/src/lib.rs
use serde::{Deserialize, Serialize};

// TA UUID - 生产环境中应使用uuidgen生成
pub const TA_AIRACCOUNT_UUID: &str = "12345678-1234-5678-9abc-123456789012";

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum Command {
    CreateWallet = 0x1000,
    GetPublicKey = 0x1001,
    SignTransaction = 0x1002,
    VerifyFingerprint = 0x1003,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SignTransactionRequest {
    pub transaction_hash: [u8; 32],
    pub derivation_path: String,
    pub fingerprint_hash: [u8; 32],
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SignTransactionResponse {
    pub signature: Vec<u8>,
    pub recovery_id: u8,
}
```

### 4.3 实现TA核心逻辑

```rust
// ta/src/main.rs
#![no_std]
#![no_main]

extern crate alloc;
use alloc::{vec::Vec, string::String};

use optee_utee::{
    ta_close_session, ta_create, ta_destroy, ta_invoke_command,
    ta_open_session, trace_println, ErrorKind, Parameters, Result,
};

use shared::{Command, SignTransactionRequest, SignTransactionResponse};

static mut MASTER_KEY: Option<[u8; 32]> = None;

#[ta_create]
fn create() -> Result<()> {
    trace_println!("AirAccount TA: Created");
    Ok(())
}

#[ta_open_session]
fn open_session(_params: &mut Parameters) -> Result<()> {
    trace_println!("AirAccount TA: Session Opened");
    Ok(())
}

#[ta_close_session]
fn close_session() {
    trace_println!("AirAccount TA: Session Closed");
}

#[ta_destroy]
fn destroy() {
    trace_println!("AirAccount TA: Destroyed");
}

#[ta_invoke_command]
fn invoke_command(cmd_id: u32, params: &mut Parameters) -> Result<()> {
    trace_println!("AirAccount TA: Invoke command {}", cmd_id);
    
    match cmd_id {
        x if x == Command::CreateWallet as u32 => create_wallet(params),
        x if x == Command::GetPublicKey as u32 => get_public_key(params),
        x if x == Command::SignTransaction as u32 => sign_transaction(params),
        x if x == Command::VerifyFingerprint as u32 => verify_fingerprint(params),
        _ => Err(ErrorKind::BadParameters.into()),
    }
}

fn create_wallet(params: &mut Parameters) -> Result<()> {
    trace_println!("Creating new wallet...");
    
    // 生成主私钥（生产环境应使用硬件随机数生成器）
    let master_key = generate_master_key()?;
    
    // 存储到安全存储（这里简化为全局变量）
    unsafe {
        MASTER_KEY = Some(master_key);
    }
    
    trace_println!("Wallet created successfully");
    Ok(())
}

fn sign_transaction(params: &mut Parameters) -> Result<()> {
    let p0 = unsafe { params.get(0).as_memref().unwrap() };
    
    // 反序列化请求
    let request: SignTransactionRequest = 
        serde_json::from_slice(p0.buffer()).map_err(|_| ErrorKind::BadFormat)?;
    
    // 验证指纹
    if !verify_fingerprint_hash(&request.fingerprint_hash)? {
        return Err(ErrorKind::AccessDenied.into());
    }
    
    // 获取主私钥
    let master_key = unsafe {
        MASTER_KEY.ok_or(ErrorKind::ItemNotFound)?
    };
    
    // 派生私钥
    let private_key = derive_private_key(&master_key, &request.derivation_path)?;
    
    // 执行签名
    let signature = ecdsa_sign(&private_key, &request.transaction_hash)?;
    
    // 构造响应
    let response = SignTransactionResponse {
        signature: signature.to_vec(),
        recovery_id: 0, // 需要实际计算
    };
    
    // 序列化并返回
    let response_bytes = serde_json::to_vec(&response)
        .map_err(|_| ErrorKind::BadFormat)?;
    
    let p1 = unsafe { params.get(1).as_memref().unwrap() };
    p1.buffer_mut()[..response_bytes.len()].copy_from_slice(&response_bytes);
    p1.set_updated_size(response_bytes.len());
    
    trace_println!("Transaction signed successfully");
    Ok(())
}

// 辅助函数实现
fn generate_master_key() -> Result<[u8; 32]> {
    // 使用OP-TEE的随机数生成器
    use optee_utee::Random;
    let mut key = [0u8; 32];
    Random::generate(&mut key);
    Ok(key)
}

fn derive_private_key(master_key: &[u8; 32], path: &str) -> Result<[u8; 32]> {
    // 实现BIP32派生（这里简化）
    use optee_utee::{Digest, DigestAlgorithm};
    let mut hasher = Digest::allocate(DigestAlgorithm::Sha256)
        .map_err(|_| ErrorKind::Generic)?;
    
    hasher.update(master_key).map_err(|_| ErrorKind::Generic)?;
    hasher.update(path.as_bytes()).map_err(|_| ErrorKind::Generic)?;
    
    let mut derived_key = [0u8; 32];
    hasher.do_final(&mut derived_key).map_err(|_| ErrorKind::Generic)?;
    
    Ok(derived_key)
}

fn ecdsa_sign(private_key: &[u8; 32], hash: &[u8; 32]) -> Result<[u8; 64]> {
    // 使用OP-TEE的ECDSA实现
    use optee_utee::{AsymmetricOperation, AsymmetricAlgorithm};
    
    // 这里需要实际的ECDSA实现
    // 为了演示，返回模拟签名
    let mut signature = [0u8; 64];
    for (i, &b) in hash.iter().enumerate() {
        if i < 64 {
            signature[i] = b;
        }
    }
    
    Ok(signature)
}

fn verify_fingerprint_hash(fp_hash: &[u8; 32]) -> Result<bool> {
    // 实现指纹验证逻辑
    // 这里简化为总是返回true
    trace_println!("Fingerprint verified");
    Ok(true)
}

fn get_public_key(params: &mut Parameters) -> Result<()> {
    // 实现获取公钥的逻辑
    Ok(())
}

fn verify_fingerprint(params: &mut Parameters) -> Result<()> {
    // 实现指纹验证的逻辑
    Ok(())
}
```

## 5. CA开发实践

### 5.1 实现TEE客户端

```rust
// host/src/main.rs
use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use optee_teec::{Context, Operation, ParamType, Session, Uuid};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use shared::{Command, SignTransactionRequest, SignTransactionResponse, TA_AIRACCOUNT_UUID};

pub struct TEEClient {
    context: Arc<Mutex<Context>>,
}

impl TEEClient {
    pub fn new() -> optee_teec::Result<Self> {
        let context = Context::new()?;
        Ok(Self {
            context: Arc::new(Mutex::new(context)),
        })
    }
    
    pub async fn create_wallet(&self) -> optee_teec::Result<()> {
        let ctx = self.context.lock().await;
        let uuid = Uuid::parse_str(TA_AIRACCOUNT_UUID).unwrap();
        let mut session = ctx.open_session(uuid)?;
        
        let mut operation = Operation::new(0, ParamType::None, ParamType::None, ParamType::None);
        
        session.invoke_command(Command::CreateWallet as u32, &mut operation)?;
        Ok(())
    }
    
    pub async fn sign_transaction(
        &self,
        request: SignTransactionRequest,
    ) -> optee_teec::Result<SignTransactionResponse> {
        let ctx = self.context.lock().await;
        let uuid = Uuid::parse_str(TA_AIRACCOUNT_UUID).unwrap();
        let mut session = ctx.open_session(uuid)?;
        
        // 序列化请求
        let request_bytes = serde_json::to_vec(&request).unwrap();
        
        // 准备响应缓冲区
        let mut response_buffer = vec![0u8; 1024];
        
        let mut operation = Operation::new(
            0,
            ParamType::MemrefTempInput,
            ParamType::MemrefTempOutput,
            ParamType::None,
        );
        
        operation.set_param(0, request_bytes.as_slice(), None);
        operation.set_param(1, response_buffer.as_mut_slice(), None);
        
        session.invoke_command(Command::SignTransaction as u32, &mut operation)?;
        
        // 反序列化响应
        let response_len = operation.param(1).unwrap().updated_size();
        let response: SignTransactionResponse = 
            serde_json::from_slice(&response_buffer[..response_len]).unwrap();
        
        Ok(response)
    }
}

// Web API处理器
#[derive(Deserialize)]
struct SignRequest {
    transaction_hash: String,
    derivation_path: String,
    fingerprint_data: String,
}

#[derive(Serialize)]
struct SignResponse {
    signature: String,
    recovery_id: u8,
}

async fn sign_handler(
    State(tee_client): State<Arc<TEEClient>>,
    Json(req): Json<SignRequest>,
) -> Result<Json<SignResponse>, StatusCode> {
    // 解析十六进制字符串
    let tx_hash = hex::decode(&req.transaction_hash)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let fp_data = hex::decode(&req.fingerprint_data)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    if tx_hash.len() != 32 || fp_data.len() != 32 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // 准备TEE请求
    let mut tx_hash_array = [0u8; 32];
    let mut fp_hash_array = [0u8; 32];
    tx_hash_array.copy_from_slice(&tx_hash);
    fp_hash_array.copy_from_slice(&fp_data);
    
    let tee_request = SignTransactionRequest {
        transaction_hash: tx_hash_array,
        derivation_path: req.derivation_path,
        fingerprint_hash: fp_hash_array,
    };
    
    // 调用TEE签名
    let tee_response = tee_client.sign_transaction(tee_request).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(SignResponse {
        signature: hex::encode(tee_response.signature),
        recovery_id: tee_response.recovery_id,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化TEE客户端
    let tee_client = Arc::new(TEEClient::new()?);
    
    // 创建钱包（仅在首次运行时）
    tee_client.create_wallet().await?;
    
    // 构建Web服务
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/sign", post(sign_handler))
        .with_state(tee_client);
    
    println!("AirAccount TEE Service starting on 0.0.0.0:8080");
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
```

## 6. 调试与测试

### 6.1 构建配置

```toml
# Cargo.toml (项目根目录)
[workspace]
members = [
    "packages/shared",
    "packages/ta-arm-trustzone", 
    "packages/client-tauri",
]

[workspace.dependencies]
optee-teec = "0.4"
optee-utee = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
hex = "0.4"
tokio = { version = "1.0", features = ["full"] }
axum = "0.7"
```

### 6.2 调试技巧

#### GDB调试

```bash
# 启动带GDB的QEMU
make run-qemuv8 GDB=1

# 在另一终端连接GDB
gdb-multiarch
(gdb) target remote localhost:1234
(gdb) symbol-file path/to/your/binary
(gdb) break main
(gdb) continue
```

#### 日志调试

```rust
// TA中使用trace_println!
trace_println!("Debug: value = {}", value);

// CA中使用标准日志
use log::{info, error, debug};
info!("Transaction signed successfully");
```

## 7. 三步开发策略评估

### 7.1 当前策略优势

✅ **渐进式风险管理**: 从虚拟环境到真实硬件，逐步验证功能
✅ **成本效益**: 大部分开发在无硬件成本的QEMU中完成  
✅ **快速迭代**: QEMU重启快，调试周期短
✅ **团队协作**: 统一的开发环境，减少"在我机器上可以运行"问题

### 7.2 潜在改进建议

#### 增加虚拟硬件多样性
```bash
# 不仅测试ARMv8-A，还应测试其他架构
make optee-qemuv7    # 32位ARM
make optee-qemuarm64 # 不同的64位变体
```

#### 引入故障注入测试
```rust
// 在TA中模拟各种故障条件
fn test_power_failure_during_key_generation() {
    // 模拟突然断电
    // 验证密钥生成的原子性
}
```

### 7.3 建议的五步策略

1. **QEMU单节点开发** ✅
2. **虚拟多节点集群测试** 🆕  
3. **云端真实硬件验证** ✅
4. **小规模生产部署** ✅
5. **持续安全审计** 🆕

---

## 结论

本指南提供了从环境搭建到生产部署的完整TEE开发流程。通过遵循这个渐进式开发策略，可以在控制风险和成本的前提下，开发出安全可靠的TEE应用。

关键成功要素：
- 🔐 安全优先的设计思维
- 🧪 全面的测试覆盖  
- 📊 持续的性能监控
- 🔄 渐进式的部署策略

更多资源：
- [Apache Teaclave TrustZone SDK](https://github.com/apache/incubator-teaclave-trustzone-sdk)
- [OP-TEE Documentation](https://optee.readthedocs.io/)
- [ARM TrustZone Technology](https://developer.arm.com/ip-products/security-ip/trustzone)