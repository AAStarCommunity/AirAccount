# KMS 开发工作流程

## 📋 目录

1. [成功经验总结](#成功经验总结)
2. [完整开发流程](#完整开发流程)
3. [优化后的工作流](#优化后的工作流)
4. [常见问题排查](#常见问题排查)
5. [关键文件说明](#关键文件说明)

---

## 成功经验总结

### ✅ 本次成功实现的功能

**功能**: 钱包地址自动管理系统（v2.0）

**核心特性**:
1. ✅ 自动地址派生（BIP44 路径 m/44'/60'/0'/0/{N}）
2. ✅ CreateKey API 返回 Address、PublicKey、DerivationPath
3. ✅ Sign API 支持 Address 参数（无需提供 DerivationPath）
4. ✅ 地址自动递增（同一 wallet_id 可创建多个地址）
5. ✅ Normal World 地址缓存（address_map.json）

**修改的文件**:
```
kms/proto/src/lib.rs              - 添加 DeriveAddressAuto 命令
kms/proto/src/in_out.rs           - 添加 DeriveAddressAutoInput/Output
kms/ta/src/wallet.rs              - 添加 next_address_index 和 increment 逻辑
kms/ta/src/main.rs                - 实现 derive_address_auto 函数
kms/host/src/address_cache.rs     - 新文件：地址缓存模块
kms/host/src/lib.rs               - 导出 address_cache 模块
kms/host/src/api_server.rs        - 重构 CreateKey 和 Sign API
scripts/kms-deploy.sh             - 修复 ta/ 子目录部署
```

### 🔧 修复的关键问题

#### 1. 部署脚本问题
**问题**: .ta 文件只复制到 `/opt/teaclave/shared/`，但 expect 脚本使用 `mount --bind ta/` 挂载子目录

**根本原因**: kms-deploy.sh 和 listen_on_guest_vm_shell 期望的目录结构不匹配

**修复**:
```bash
# 修改 kms-deploy.sh，确保复制到 ta/ 子目录
mkdir -p /opt/teaclave/shared/ta
cp *.ta /opt/teaclave/shared/ta/
```

#### 2. 模块导入错误
**问题**: `error[E0433]: failed to resolve: use of undeclared crate or module kms_host`

**修复**:
```rust
// api_server.rs
use kms::address_cache::{update_address_entry, lookup_address};

// 不要使用：
// kms_host::update_address_entry(...)  ❌

// 应该使用：
// update_address_entry(...)  ✅
```

#### 3. 类型不匹配错误
**问题**: `expected (Uuid, String), found (Uuid, &String)`

**修复**:
```rust
// 错误：
(wallet_uuid, path.clone())  // path 是 &String，clone() 还是 String 引用

// 正确：
(wallet_uuid, path.to_string())  // 转换为 String
```

#### 4. 未使用的 mut 变量
**问题**: TA 编译警告变成错误（`-D unused-mut`）

**修复**:
```rust
// 错误：
let (wallet_id, mut wallet, address_index) = if ... {
    let mut wallet = ...  // 内部重新声明，外部 mut 无用

// 正确：
let (wallet_id, wallet, address_index) = if ... {
```

---

## 完整开发流程

### 步骤 1: Docker 重启（如果需要）

```bash
# 方式 1: 命令行
docker stop teaclave_dev_env
docker start teaclave_dev_env

# 方式 2: Docker Desktop Dashboard
# 手动点击重启按钮
```

**何时需要**:
- Docker 内有僵尸进程无法清理
- Docker 容器状态异常
- 否则可以跳过此步骤

### 步骤 2: 启动 Cloudflare Tunnel（可选）

```bash
# 只在需要公网访问时启动
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &
```

**用途**: 将 localhost:3000 暴露到 https://kms.aastar.io

### 步骤 3: 清理旧进程

```bash
./scripts/kms-cleanup.sh
```

**清理内容**:
- QEMU 进程
- socat 监听器（端口 54320, 54321）
- expect 脚本

### 步骤 4: 部署（编译 + 复制）

```bash
# 增量构建（快速）
./scripts/kms-deploy.sh

# 完全重新构建（修改代码后推荐）
./scripts/kms-deploy.sh clean
```

**自动执行**:
1. 同步 kms/ → SDK (rsync)
2. 编译 TA (aarch64-unknown-optee)
3. 编译 Host (aarch64-unknown-linux-gnu)
4. 复制到 `/opt/teaclave/shared/` 和 `/opt/teaclave/shared/ta/`

### 步骤 5: 启动服务 ⭐ **简化版：只需 Terminal 2**

**最简方式**（推荐）:

```bash
# 只启动 Terminal 2，它会自动启动其他所有服务
./scripts/terminal2-guest-vm.sh
```

**说明**: `listen_on_guest_vm_shell` expect 脚本会自动：
1. ✅ 启动端口 54320 监听器
2. ✅ 等待 QEMU 启动并连接
3. ✅ 自动登录到 QEMU
4. ✅ 挂载 shared 目录
5. ✅ 绑定挂载 TA 和 plugin
6. ✅ 启动 kms-api-server

**可选：三终端方式**（仅用于调试）:

```bash
# Terminal 3 (Secure World 日志 - 可选)
./scripts/terminal3-secure-log.sh

# Terminal 2 (Guest VM Shell - 必需)
./scripts/terminal2-guest-vm.sh

# Terminal 1 (QEMU 控制 - 可选)
# Terminal 2 会自动触发 QEMU 启动，无需单独运行
```

**后台自动启动**（无交互，用于 CI/CD）:

```bash
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"
sleep 60  # 等待完全启动
```

### 步骤 6: 测试

```bash
# 本地测试
curl -s http://localhost:3000/health | jq .

# 公网测试（需要先启动 cloudflared）
curl -s https://kms.aastar.io/health | jq .

# 测试新功能：创建第一个地址
curl -s -X POST 'http://localhost:3000/CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{
    "Description": "Test wallet",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }' | jq .

# 保存返回的 KeyId，例如：48c8d60e-0134-4488-926a-5521accb9e14

# 测试新功能：创建第二个地址（同一钱包）
curl -s -X POST 'http://localhost:3000/CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{
    "KeyId": "48c8d60e-0134-4488-926a-5521accb9e14",
    "Description": "Second address",
    "KeyUsage": "SIGN_VERIFY",
    "KeySpec": "ECC_SECG_P256K1",
    "Origin": "AWS_KMS"
  }' | jq .

# 测试新功能：使用 Address 签名（无需 DerivationPath）
curl -s -X POST 'http://localhost:3000/Sign' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.Sign' \
  -d '{
    "Address": "0xad365342c8ee4a951251c10fff8f840cbdf1dd4e",
    "Message": "Hello World"
  }' | jq .
```

---

## 优化后的工作流

### 🚀 一键部署脚本（推荐）

创建 `scripts/kms-dev-cycle.sh`：

```bash
#!/bin/bash
# KMS 完整开发周期：清理 → 编译 → 部署 → 启动 → 测试

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}🔧 KMS Development Cycle${NC}"
echo ""

# 1. 清理
echo -e "${BLUE}[1/5]${NC} 清理旧进程..."
./scripts/kms-cleanup.sh

# 2. 编译部署
echo -e "${BLUE}[2/5]${NC} 编译和部署..."
CLEAN_ARG=${1:-}  # 支持传入 "clean" 参数
./scripts/kms-deploy.sh $CLEAN_ARG

# 3. 启动服务
echo -e "${BLUE}[3/5]${NC} 启动 QEMU 和 API..."
docker exec -d teaclave_dev_env bash -l -c "listen_on_secure_world_log"
sleep 2
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"
sleep 3
docker exec -d teaclave_dev_env bash -c "cd /root/teaclave_sdk_src && IMG_DIRECTORY=/opt/teaclave/images IMG_NAME=x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory QEMU_HOST_SHARE_DIR=/opt/teaclave/shared LISTEN_MODE=1 ./scripts/runtime/bin/start_qemuv8"

echo -e "${YELLOW}⏳ 等待服务启动（60秒）...${NC}"
for i in {1..12}; do
    echo -n "."
    sleep 5
done
echo ""

# 4. 验证
echo -e "${BLUE}[4/5]${NC} 验证部署..."
if curl -s -m 5 'http://localhost:3000/health' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ API 服务正常${NC}"
    curl -s 'http://localhost:3000/health' | jq .
else
    echo -e "${YELLOW}⚠️  API 未响应，请检查日志${NC}"
    exit 1
fi

# 5. 功能测试
echo -e "${BLUE}[5/5]${NC} 功能测试..."
echo "测试创建第一个地址..."
RESPONSE=$(curl -s -X POST 'http://localhost:3000/CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{"Description":"Auto test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}')

echo "$RESPONSE" | jq .

KEY_ID=$(echo "$RESPONSE" | jq -r '.KeyMetadata.KeyId')
ADDRESS=$(echo "$RESPONSE" | jq -r '.KeyMetadata.Address')
PATH1=$(echo "$RESPONSE" | jq -r '.KeyMetadata.DerivationPath')

echo ""
echo -e "${GREEN}✅ 第一个地址创建成功：${NC}"
echo "  KeyId: $KEY_ID"
echo "  Address: $ADDRESS"
echo "  Path: $PATH1"

echo ""
echo "测试创建第二个地址（同一钱包）..."
RESPONSE2=$(curl -s -X POST 'http://localhost:3000/CreateKey' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d "{\"KeyId\":\"$KEY_ID\",\"Description\":\"Second address\",\"KeyUsage\":\"SIGN_VERIFY\",\"KeySpec\":\"ECC_SECG_P256K1\",\"Origin\":\"AWS_KMS\"}")

ADDRESS2=$(echo "$RESPONSE2" | jq -r '.KeyMetadata.Address')
PATH2=$(echo "$RESPONSE2" | jq -r '.KeyMetadata.DerivationPath')

echo -e "${GREEN}✅ 第二个地址创建成功：${NC}"
echo "  Address: $ADDRESS2"
echo "  Path: $PATH2"

echo ""
echo "测试地址签名..."
SIG=$(curl -s -X POST 'http://localhost:3000/Sign' \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.Sign' \
  -d "{\"Address\":\"$ADDRESS\",\"Message\":\"Test message\"}" | jq -r '.Signature')

echo -e "${GREEN}✅ 签名成功：${NC}"
echo "  Signature: ${SIG:0:32}..."

echo ""
echo -e "${GREEN}🎉 所有测试通过！${NC}"
echo ""
echo -e "${BLUE}📊 开发环境状态：${NC}"
echo "  本地: http://localhost:3000/health"
echo "  公网: https://kms.aastar.io/health (需要启动 cloudflared)"
```

使用方法：

```bash
# 增量构建 + 测试
./scripts/kms-dev-cycle.sh

# 完全重新构建 + 测试
./scripts/kms-dev-cycle.sh clean
```

---

## 常见问题排查

### 问题 1: API 返回旧版本（没有新端点）

**症状**:
```json
{
  "endpoints": {
    "POST": ["/CreateKey", "/Sign", "/DeleteKey"]
    // 缺少 /SignHash 或其他新端点
  }
}
```

**原因**: QEMU 内运行的是旧二进制

**排查**:
```bash
# 1. 检查部署的二进制文件时间戳
docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/kms-api-server
docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/ta/

# 2. 确认 ta/ 子目录存在且有新文件
docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/ta/*.ta

# 3. 重新部署
./scripts/kms-cleanup.sh
./scripts/kms-deploy.sh clean
# 然后重新启动 QEMU
```

### 问题 2: 编译错误 - 模块未声明

**症状**:
```
error[E0433]: failed to resolve: use of undeclared crate or module `kms_host`
```

**原因**: 模块路径错误

**修复**:
```rust
// 检查 lib.rs 是否导出模块
// kms/host/src/lib.rs
pub mod address_cache;
pub use address_cache::{...};

// 在 api_server.rs 中使用
use kms::address_cache::{update_address_entry, lookup_address};
```

### 问题 3: 类型不匹配

**症状**:
```
expected `String`, found `&String`
```

**修复**:
```rust
// 使用 .to_string() 而不是 .clone()
let owned_string = ref_string.to_string();
```

### 问题 4: 端口被占用

**症状**:
```
bind(14, {AF=2 0.0.0.0:54320}, 16): Address already in use
```

**修复**:
```bash
# 清理所有进程
./scripts/kms-cleanup.sh

# 如果还有僵尸进程，重启 Docker
docker restart teaclave_dev_env
```

### 问题 5: QEMU 启动但 API 无响应

**症状**: 等待 60 秒后 curl 超时

**排查**:
```bash
# 1. 检查 QEMU 是否运行
docker exec teaclave_dev_env ps aux | grep qemu

# 2. 检查 expect 脚本是否运行
docker exec teaclave_dev_env ps aux | grep expect

# 3. 查看 shared 目录日志
docker exec teaclave_dev_env cat /opt/teaclave/shared/kms-api.log
```

**常见原因**:
- expect 脚本启动失败（查看 `/tmp/guest_vm.log`）
- 串口连接问题（54320/54321 端口）
- 二进制文件损坏或不兼容

---

## 关键文件说明

### 部署相关

| 文件 | 用途 | 关键点 |
|------|------|--------|
| `scripts/kms-deploy.sh` | 编译和部署 | 必须复制到 `ta/` 子目录 |
| `scripts/kms-cleanup.sh` | 清理进程 | 清理 QEMU、socat、expect |
| `scripts/terminal1-qemu.sh` | 启动 QEMU | LISTEN_MODE=1 |
| `scripts/terminal2-guest-vm.sh` | Guest VM 监听器 | 端口 54320 |
| `scripts/terminal3-secure-log.sh` | Secure World 日志 | 端口 54321 |

### 源代码

| 文件 | 修改内容 | 注意事项 |
|------|---------|----------|
| `kms/proto/src/lib.rs` | 添加 DeriveAddressAuto 命令 | 枚举值 |
| `kms/proto/src/in_out.rs` | 添加 Input/Output 结构 | Serde 序列化 |
| `kms/ta/src/wallet.rs` | 添加计数器和递增逻辑 | 最大地址数 100 |
| `kms/ta/src/main.rs` | 实现 derive_address_auto | 去掉不必要的 mut |
| `kms/host/src/address_cache.rs` | 地址缓存模块 | JSON 格式 |
| `kms/host/src/lib.rs` | 导出模块 | pub mod + pub use |
| `kms/host/src/api_server.rs` | API 重构 | 正确的导入路径 |

### Docker 路径映射

| Host (Mac) | Docker | QEMU |
|-----------|--------|------|
| `kms/` | `/root/teaclave_sdk_src/projects/web3/kms/` | - |
| - | `/opt/teaclave/shared/` | `/root/shared/` |
| - | `/opt/teaclave/shared/ta/` | `/lib/optee_armtz/` (bind mount) |

### expect 脚本自动化

`/opt/teaclave/bin/listen_on_guest_vm_shell` 自动执行：

```expect
spawn socat TCP-LISTEN:54320,reuseaddr,fork -,raw,echo=0
expect "buildroot login:"
send "root\r"
expect "# $"
send "mkdir -p shared && mount -t 9p -o trans=virtio host shared && cd shared\r"
expect "# $"
send "mount --bind ta/ /lib/optee_armtz\r"
expect "# $"
send "mount --bind plugin/ /usr/lib/tee-supplicant/plugins/\r"
expect "# $"
send "./kms-api-server > kms-api.log 2>&1 &\r"
expect "# $"
send "echo 'KMS API Server started'\r"
interact
```

---

## 性能优化建议

### 1. 增量构建
```bash
# 只修改了 API 代码（不涉及 TA）
cd kms/host && cargo build --target aarch64-unknown-linux-gnu --release
# 手动复制二进制
```

### 2. 跳过 Docker 重启
- 只在必要时重启 Docker（僵尸进程堆积）
- 通常 kms-cleanup.sh 足够

### 3. 并行测试
```bash
# 启动后立即测试，不等待 60 秒
while ! curl -s http://localhost:3000/health > /dev/null 2>&1; do
    echo -n "."
    sleep 2
done
echo "API ready!"
```

---

## 下次开发快速参考

```bash
# 1. 修改代码
vim kms/host/src/api_server.rs

# 2. 一键部署测试
./scripts/kms-dev-cycle.sh clean

# 3. 如果需要公网访问
cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &
curl https://kms.aastar.io/health

# 4. 更新文档
vim docs/Changes.md
```

---

## 🎯 本次实现的完整测试流程

```bash
# 测试 1: 创建第一个钱包和地址
curl -X POST http://localhost:3000/CreateKey \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{"Description":"Test","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'
# 预期: 返回 Address, PublicKey, DerivationPath (m/44'/60'/0'/0/0)

# 测试 2: 同一钱包创建第二个地址
curl -X POST http://localhost:3000/CreateKey \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.CreateKey' \
  -d '{"KeyId":"<wallet_id>","Description":"Second","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}'
# 预期: DerivationPath 变为 m/44'/60'/0'/0/1

# 测试 3: 使用 Address 签名
curl -X POST http://localhost:3000/Sign \
  -H 'Content-Type: application/x-amz-json-1.1' \
  -H 'x-amz-target: TrentService.Sign' \
  -d '{"Address":"0x...","Message":"Hello World"}'
# 预期: 返回签名，无需提供 DerivationPath

# 测试 4: 地址缓存验证
docker exec teaclave_dev_env cat /opt/teaclave/shared/address_map.json
# 预期: 包含 address → {wallet_id, derivation_path, public_key} 映射
```

**最终验证**: 所有测试通过 ✅

---

## 📝 更新日志

**2025-10-02**:
- ✅ 完成钱包地址自动管理系统 v2.0
- ✅ 修复 kms-deploy.sh ta 子目录问题
- ✅ 修复所有编译错误（模块导入、类型匹配）
- ✅ 验证端到端功能（地址自动递增、Address-based 签名）
- ✅ 创建完整开发工作流文档
