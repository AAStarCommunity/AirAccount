# AirAccount OP-TEE 开发快速启动指南

本指南帮助开发者快速搭建 AirAccount 的 OP-TEE 开发环境。

## 🚀 一键安装（推荐）

### 步骤 1：克隆仓库
```bash
git clone https://github.com/your-org/AirAccount.git
cd AirAccount
```

### 步骤 2：安装依赖
```bash
# 一键安装所有必需依赖
./scripts/install_dependencies.sh
```

### 步骤 3：初始化子模块
```bash
# 初始化 Teaclave TrustZone SDK
git submodule update --init --recursive third_party/incubator-teaclave-trustzone-sdk
```

### 步骤 4：构建 OP-TEE 环境
```bash
# 加载环境配置
source scripts/setup_optee_env.sh

# 构建 OP-TEE 库
cd third_party/incubator-teaclave-trustzone-sdk
./build_optee_libraries.sh "$OPTEE_DIR"

# 修复库文件路径（macOS 特有）
cp -r "$OPTEE_DIR/optee_client/out/libteec/"* "$OPTEE_CLIENT_EXPORT/usr/lib/"
```

### 步骤 5：验证安装
```bash
cd /path/to/AirAccount
./scripts/verify_optee_setup.sh
```

### 步骤 6：运行测试
```bash
./scripts/test_all.sh
```

## 🎯 快速验证

运行 Mock 版本验证基础架构：

```bash
cd packages/mock-hello
cargo run --bin mock-ca test
```

如果看到以下输出，说明基础环境正常：
```
🧪 === AirAccount Mock TA-CA Communication Tests ===

Test 1 - Hello World: ✅ PASS
Test 2 - Echo Message: ✅ PASS  
Test 3 - Version Info: ✅ PASS
Test 4 - Wallet Creation: ✅ PASS
Test 5 - Multiple Operations: ✅ PASS (20/20 operations)

🎉 === Test Suite Completed ===
```

## 📁 项目结构

```
AirAccount/
├── docs/                          # 文档
│   ├── OP-TEE-Development-Setup.md   # 详细安装指南
│   └── Quick-Start-Guide.md          # 本指南
├── scripts/                       # 自动化脚本
│   ├── install_dependencies.sh      # 依赖安装
│   ├── setup_optee_env.sh          # 环境配置
│   ├── verify_optee_setup.sh       # 环境验证
│   ├── build_all.sh                # 完整构建
│   └── test_all.sh                 # 完整测试
├── packages/
│   ├── mock-hello/                 # Mock TA-CA 通信框架
│   └── core-logic/                 # 安全核心逻辑
└── third_party/
    └── incubator-teaclave-trustzone-sdk/  # OP-TEE SDK
```

## 🔧 日常开发工作流

### 1. 开始开发会话
```bash
cd /path/to/AirAccount
source scripts/setup_optee_env.sh
```

### 2. 快速测试
```bash
# 运行 Mock 版本测试
cd packages/mock-hello
cargo run --bin mock-ca interactive

# 在交互模式中尝试命令：
MockCA> hello
MockCA> echo "test message"  
MockCA> version
MockCA> wallet
MockCA> test
MockCA> quit
```

### 3. 完整构建
```bash
./scripts/build_all.sh
```

### 4. 运行所有测试
```bash
./scripts/test_all.sh
```

## 🛠️ 开发命令速查

### 环境管理
```bash
# 加载开发环境
source scripts/setup_optee_env.sh

# 验证环境状态
./scripts/verify_optee_setup.sh

# 查看环境变量
echo $OPTEE_DIR
echo $TA_DEV_KIT_DIR
echo $CROSS_COMPILE64
```

### Mock 开发
```bash
cd packages/mock-hello

# 构建
cargo build --release

# 运行各种命令
cargo run --bin mock-ca hello
cargo run --bin mock-ca echo "message"
cargo run --bin mock-ca version
cargo run --bin mock-ca create-wallet
cargo run --bin mock-ca test

# 交互模式
cargo run --bin mock-ca interactive
```

### OP-TEE 客户端开发
```bash
# Hello World 客户端
cd third_party/incubator-teaclave-trustzone-sdk/examples/hello_world-rs/host
cargo build --target aarch64-unknown-linux-gnu --release

# eth_wallet 客户端
cd ../../../projects/web3/eth_wallet/host  
cargo build --target aarch64-unknown-linux-gnu --release
```

### 测试命令
```bash
# 单独运行不同类型的测试
cargo test                    # 单元测试
cargo test --workspace        # 工作区测试  
cargo clippy --workspace      # 代码检查
cargo fmt --all -- --check    # 格式检查
```

## ⚠️ 已知问题

### TA 构建问题
当前 TA (Trusted Application) 构建存在 `optee-utee-sys` 的 std 依赖问题：

```bash
# TA 构建会失败（已知问题）
cd third_party/incubator-teaclave-trustzone-sdk/examples/hello_world-rs/ta
TA_DEV_KIT_DIR="$TA_DEV_KIT_DIR" \
cargo +nightly-2024-05-15 build \
--target "../../../aarch64-unknown-optee.json" \
-Z build-std=core,alloc,std --release
```

**解决方案**：
1. 使用 Mock 版本进行开发和测试（推荐）
2. 等待 Teaclave SDK 上游修复
3. 客户端开发不受影响，可以正常进行

### macOS 特定问题
- `cp -d` 参数不支持：已通过手动复制库文件解决
- `rmdir --ignore-fail-on-non-empty` 不支持：不影响核心功能

## 🚀 开发建议

### 新手开发者
1. **从 Mock 版本开始**：使用 `packages/mock-hello` 学习 TA-CA 通信模式
2. **理解架构**：研读 `docs/ETH_Wallet_Deep_Analysis.md`
3. **跟随测试**：运行 `./scripts/test_all.sh` 了解测试覆盖

### 高级开发者
1. **直接使用 OP-TEE 客户端**：开发真实的安全应用
2. **扩展安全模块**：在 `packages/core-logic` 中增强安全功能
3. **贡献上游**：帮助修复 Teaclave SDK 的 TA 构建问题

## 📞 获取帮助

- **详细文档**：参见 `docs/OP-TEE-Development-Setup.md`
- **环境问题**：运行 `./scripts/verify_optee_setup.sh` 获取诊断信息
- **构建问题**：检查 `./scripts/build_all.sh` 的输出日志
- **测试失败**：查看 `./scripts/test_all.sh` 的详细报告

## 🎉 成功标志

如果以下命令都能成功运行，说明环境完全就绪：

```bash
# ✅ 环境验证通过
./scripts/verify_optee_setup.sh

# ✅ 构建成功
./scripts/build_all.sh

# ✅ 所有测试通过
./scripts/test_all.sh
```

现在你可以开始 AirAccount 的 TEE 应用开发了！🚀