# KMS Passkey 独立 Docker 开发环境

> 创建时间: 2025-10-02 13:52

## 概述

KMS-feat-passkey 分支的独立 Docker 开发环境,与主干分支 (teaclave_dev_env) 完全隔离,仅用于本地 Passkey 功能测试,不部署到生产环境。

## 架构

### 基础镜像
- **镜像**: `teaclave/teaclave-trustzone-emulator-std-optee-4.5.0-expand-memory:latest`
- **模式**: STD (aarch64)
- **OP-TEE**: 4.5.0

### 环境变量
```bash
TEACLAVE_TOOLCHAIN_BASE=/opt/teaclave
RUST_STD_DIR=/opt/teaclave/std
KMS_BRANCH=KMS-feat-passkey
```

### 容器配置
- **容器名**: `kms_passkey_dev`
- **镜像名**: `kms-passkey-qemu:latest`

### 端口映射
| 宿主端口 | 容器端口 | 用途 |
|---------|---------|------|
| 54320 | 54320 | Guest VM Shell |
| 54321 | 54321 | Secure World Log |
| 3001 | 3000 | KMS API Server |

### 卷挂载
```bash
/Volumes/UltraDisk/Dev2/aastar/AirAccount -> /root/kms_passkey_src
/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/teaclave-trustzone-sdk -> /root/teaclave_sdk_src
```

## 使用方法

### 1. 构建镜像
```bash
./scripts/kms-passkey-docker.sh build
```

### 2. 启动容器
```bash
./scripts/kms-passkey-docker.sh start
```

### 3. 验证环境
```bash
./scripts/kms-passkey-test.sh
```

### 4. 进入容器
```bash
./scripts/kms-passkey-docker.sh shell
```

### 5. 查看状态
```bash
./scripts/kms-passkey-docker.sh status
```

### 6. 其他命令
```bash
# 停止容器
./scripts/kms-passkey-docker.sh stop

# 重启容器
./scripts/kms-passkey-docker.sh restart

# 查看日志
./scripts/kms-passkey-docker.sh logs

# 删除容器
./scripts/kms-passkey-docker.sh remove
```

## 容器内开发

### 环境准备
容器启动后会自动执行 entrypoint 脚本,完成:
1. 加载 Cargo 环境 (`~/.cargo/env`)
2. 加载 RUST_STD_DIR (`~/.profile`)
3. 创建 Rust std 库符号链接 (`/root/kms_passkey_src/rust`)
4. 验证 STD 模式配置 (`/opt/teaclave/config/ta/active -> std/aarch64`)

### 开发流程
```bash
# 1. 进入容器
./scripts/kms-passkey-docker.sh shell

# 2. 在容器内编译 TA
cd /root/kms_passkey_src/kms/ta
source ~/.cargo/env
source ~/.profile
cargo build --release

# 3. 编译 Host
cd /root/kms_passkey_src/kms/host
cargo build --release --target aarch64-unknown-linux-gnu

# 4. 启动 QEMU (需要专门的脚本,后续实现)
```

## 与主环境的区别

| 特性 | 主环境 (teaclave_dev_env) | Passkey 环境 (kms_passkey_dev) |
|-----|--------------------------|-------------------------------|
| 容器名 | teaclave_dev_env | kms_passkey_dev |
| 用途 | 生产开发 | 本地测试 (Passkey) |
| API 端口 | 3000 | 3001 |
| 部署脚本 | kms-deploy.sh | 独立管理 |
| 代码隔离 | 主干分支 | KMS-feat-passkey 分支 |

## 已知问题

### Signature 版本冲突
当前 TA 编译存在依赖冲突:
- `bip32 v0.3.0` (HD 钱包) → 需要 `signature v1.3-1.4`
- `p256 v0.11` (Passkey) → 需要 `signature v1.5-1.6`

**解决方案**:
- 升级 `bip32` 到 v0.5
- 或将 Passkey 验证移到 Host CA 层

## 技术栈

### Rust Toolchain
- **版本**: 1.80.0-nightly (2024-05-14)
- **Target**: aarch64-unknown-optee (TA), aarch64-unknown-linux-gnu (Host)
- **构建器**: xargo (STD 模式)

### 依赖项
- OP-TEE: 4.5.0
- Teaclave TrustZone SDK: latest
- Docker: 支持 linux/amd64 和 linux/arm64

## 参考文档

- [OP-TEE QEMU v8](https://optee.readthedocs.io/en/latest/building/devices/qemu.html#qemu-v8)
- [OP-TEE with Rust](https://optee.readthedocs.io/en/latest/building/optee_with_rust.html)
- [Teaclave TrustZone SDK](https://teaclave.apache.org/trustzone-sdk-docs/emulate-and-dev-in-docker.md)
- [Changes.md](../docs/Changes.md) - 完整变更日志
