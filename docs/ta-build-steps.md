# TA构建步骤记录

*创建时间: 2025-09-29*

## 🎯 TA构建成功步骤总结

### 1. 环境准备
使用Teaclave std模式Docker镜像：
```bash
docker run -d --name kms-build -v $(pwd):/workspace -w /workspace/kms/ta \
  teaclave/teaclave-trustzone-emulator-std-optee-4.5.0-expand-memory:latest \
  tail -f /dev/null
```

### 2. 安装必需组件
```bash
docker exec kms-build bash -c "
source ~/.cargo/env &&
rustup component add --toolchain nightly-2024-05-15-x86_64-unknown-linux-gnu rustfmt clippy
"
```

### 3. 设置环境变量
```bash
export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64
export RUST_TARGET_PATH=/opt/teaclave/std
export CROSS_COMPILE=aarch64-linux-gnu-
```

### 4. 修复代码问题
修复`kms/ta/src/lib.rs`中的clippy错误：
```rust
// 删除未使用的import
// use alloc::string::String;  // 删除这行
```

### 5. 执行构建
```bash
docker exec kms-build bash -c "
source ~/.cargo/env &&
export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64 &&
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64 &&
export RUST_TARGET_PATH=/opt/teaclave/std &&
export CROSS_COMPILE=aarch64-linux-gnu- &&
make
"
```

### 6. 构建结果
- ✅ TA成功编译，生成文件：`target/aarch64-unknown-optee/release/be2dc9a0-02b4-4b33-ba21-9964dbdf1573.ta`
- 📦 文件大小：595KB
- 🔐 已签名并可用于OP-TEE环境

### 7. 部署TA
```bash
docker exec kms-build bash -c "
mkdir -p /lib/optee_armtz &&
cp target/aarch64-unknown-optee/release/be2dc9a0-02b4-4b33-ba21-9964dbdf1573.ta /lib/optee_armtz/
"
```

## 🔧 关键技术点

### 构建系统
- 使用Makefile调用cargo build
- 必需rustfmt和clippy组件
- 使用std模式（不是no-std）

### 依赖配置
- 在`Xargo.toml`中使用绝对路径指向Docker内的Teaclave路径
- `Cargo.toml`指向`third_party/teaclave-trustzone-sdk`

### eth_wallet TA功能
包含4个核心命令：
1. `CreateWallet` - 创建钱包
2. `RemoveWallet` - 删除钱包
3. `DeriveAddress` - 推导地址
4. `SignTransaction` - 签名交易

### 构建输出
```
SIGN => be2dc9a0-02b4-4b33-ba21-9964dbdf1573
```
这表明TA已成功构建并签名。

## 🚨 常见问题解决

### 问题1：缺少组件
```
error: 'cargo-fmt' is not installed
```
**解决**：安装rustfmt和clippy组件

### 问题2：未使用import
```
error: unused import: `alloc::string::String`
```
**解决**：删除未使用的import

### 问题3：路径错误
```
couldn't canonicalize ../../../../rust/libc
```
**解决**：在Xargo.toml中使用绝对Docker路径

## 🏗️ Host应用构建步骤

### 1. 创建Cargo配置
在host目录中创建正确的链接器配置：
```bash
docker exec kms-build bash -c "
cd /workspace/kms/host &&
mkdir -p .cargo &&
cat > .cargo/config.toml << 'EOF'
[target.aarch64-unknown-linux-gnu]
linker = \"aarch64-linux-gnu-gcc\"
EOF
"
```

### 2. 设置Host构建环境
```bash
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64
export TEEC_EXPORT=$OPTEE_CLIENT_EXPORT
export PKG_CONFIG_PATH=$OPTEE_CLIENT_EXPORT/lib/pkgconfig
```

### 3. 执行Host构建
```bash
docker exec kms-build bash -c "
source ~/.cargo/env &&
cd /workspace/kms/host &&
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64 &&
export TEEC_EXPORT=\$OPTEE_CLIENT_EXPORT &&
export PKG_CONFIG_PATH=\$OPTEE_CLIENT_EXPORT/lib/pkgconfig &&
cargo clean &&
cargo build --release --target aarch64-unknown-linux-gnu
"
```

### 4. Host构建结果
- ✅ Host应用成功编译：`target/aarch64-unknown-linux-gnu/release/eth_wallet-rs`
- 📦 文件大小：921KB
- 🏗️ ARM64架构ELF可执行文件
- 🔗 正确链接到OP-TEE Client库

## 🎉 完整构建成功总结

### ✅ 构建成果
1. **TA构建成功**：
   - 文件：`be2dc9a0-02b4-4b33-ba21-9964dbdf1573.ta` (595KB)
   - 已部署到：`/lib/optee_armtz/`
   - 包含4个核心TA命令

2. **Host应用构建成功**：
   - 文件：`eth_wallet-rs` (921KB)
   - ARM64架构可执行文件
   - 正确链接OP-TEE Client

### 🔧 关键技术解决方案

#### TA构建关键点
- 使用Teaclave std模式Docker镜像
- 安装rustfmt和clippy组件
- 修复clippy错误（删除未使用import）
- 使用绝对路径配置Xargo.toml

#### Host构建关键点
- 创建`.cargo/config.toml`指定正确链接器
- 使用`aarch64-linux-gnu-gcc`作为链接器
- 设置OP-TEE Client环境变量
- 清理构建缓存重新编译

### 🚀 构建命令快速参考

**一键构建TA**：
```bash
docker exec kms-build bash -c "
source ~/.cargo/env &&
cd /workspace/kms/ta &&
export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64 &&
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64 &&
export RUST_TARGET_PATH=/opt/teaclave/std &&
export CROSS_COMPILE=aarch64-linux-gnu- &&
make
"
```

**一键构建Host**：
```bash
docker exec kms-build bash -c "
source ~/.cargo/env &&
cd /workspace/kms/host &&
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64 &&
export TEEC_EXPORT=\$OPTEE_CLIENT_EXPORT &&
export PKG_CONFIG_PATH=\$OPTEE_CLIENT_EXPORT/lib/pkgconfig &&
cargo build --release --target aarch64-unknown-linux-gnu
"
```

## 🌟 HelloWorld API扩展示例

### 添加新API的完整流程

我们成功添加了一个HelloWorld API作为扩展示例，展示了如何为eth_wallet添加新功能。

#### 1. 修改Proto定义 (proto/src/lib.rs)
```rust
// 在Command枚举中添加新命令
pub enum Command {
    CreateWallet,
    RemoveWallet,
    DeriveAddress,
    SignTransaction,
    HelloWorld,  // 新增
    #[default]
    Unknown,
}
```

#### 2. 添加输入输出结构 (proto/src/in_out.rs)
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HelloWorldInput {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HelloWorldOutput {
    pub message: String,
}
```

#### 3. 在TA中实现函数 (ta/src/main.rs)
```rust
fn hello_world(input: &proto::HelloWorldInput) -> Result<proto::HelloWorldOutput> {
    dbg_println!("[+] Hello world called with name: {}", input.name);

    let message = format!(
        "Hello, {}! This message is from TEE (Trusted Execution Environment).",
        input.name
    );

    dbg_println!("[+] Hello world response: {}", message);

    Ok(proto::HelloWorldOutput { message })
}

// 在handle_invoke中添加处理
match command {
    // ... 其他命令
    Command::HelloWorld => process(serialized_input, hello_world),
    _ => bail!("Unsupported command"),
}
```

#### 4. 在Host中添加API和CLI (host/src/main.rs, host/src/cli.rs)
```rust
// 在main.rs中添加公共函数
pub fn hello_world(name: &str) -> Result<String> {
    let input = proto::HelloWorldInput {
        name: name.to_string(),
    };
    let serialized_output = invoke_command(
        proto::Command::HelloWorld,
        &bincode::serialize(&input)?,
    )?;
    let output: proto::HelloWorldOutput = bincode::deserialize(&serialized_output)?;
    Ok(output.message)
}

// 在cli.rs中添加CLI选项
#[derive(Debug, StructOpt)]
pub struct HelloWorldOpt {
    #[structopt(short, long, required = true)]
    pub name: String,
}

// 在Command枚举中添加
pub enum Command {
    // ... 其他命令
    /// Say hello from TEE.
    #[structopt(name = "hello")]
    HelloWorld(HelloWorldOpt),
}

// 在main函数中添加处理
match args.command {
    // ... 其他命令
    cli::Command::HelloWorld(opt) => {
        let message = hello_world(&opt.name)?;
        println!("{}", message);
    }
}
```

#### 5. 构建和测试
使用相同的构建命令：
```bash
# 构建TA
docker exec kms-build bash -c "
source ~/.cargo/env &&
cd /workspace/kms/ta &&
export TA_DEV_KIT_DIR=/opt/teaclave/optee/optee_os/out/arm-plat-vexpress/export-ta_arm64 &&
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64 &&
export RUST_TARGET_PATH=/opt/teaclave/std &&
export CROSS_COMPILE=aarch64-linux-gnu- &&
make
"

# 构建Host
docker exec kms-build bash -c "
source ~/.cargo/env &&
cd /workspace/kms/host &&
export OPTEE_CLIENT_EXPORT=/opt/teaclave/optee/optee_client/export_arm64 &&
export TEEC_EXPORT=\$OPTEE_CLIENT_EXPORT &&
export PKG_CONFIG_PATH=\$OPTEE_CLIENT_EXPORT/lib/pkgconfig &&
cargo build --release --target aarch64-unknown-linux-gnu
"
```

### 💡 API设计最佳实践

1. **命令命名**: 使用清晰的动词+名词结构
2. **输入验证**: 在TA中验证所有输入参数
3. **错误处理**: 使用anyhow::Result进行错误传播
4. **日志记录**: 使用dbg_println!记录关键操作
5. **序列化**: 使用bincode进行高效的二进制序列化

### 🔄 保持容器状态

为了提高开发效率，我们保留了kms-build容器，包含：
- ✅ 已安装的rustfmt和clippy组件
- ✅ 配置好的交叉编译环境
- ✅ 完整的Teaclave工具链

使用现有容器：
```bash
# 检查容器状态
docker ps | grep kms-build

# 如果容器已停止，重新启动
docker start kms-build
```

## 🎯 QEMU OP-TEE 测试结果总结

*测试时间: 2025-09-29*

### ✅ 成功完成的任务

1. **TA和Host构建**: 成功构建了完整的eth_wallet系统
   - TA: `be2dc9a0-02b4-4b33-ba21-9964dbdf1573.ta` (608KB)
   - Host: `eth_wallet-rs` (945KB，包含HelloWorld API)

2. **Docker环境设置**: 创建了专用的std模式测试容器`kms-test-std`

3. **文件部署**: 成功将TA和Host文件部署到QEMU测试环境

4. **QEMU启动验证**: 确认QEMU可以启动ARM TrustZone Firmware

### 🔍 重大发现：QEMU配置问题的根本原因

**关键突破**：通过对比测试发现问题不在代码，而在QEMU启动配置！

#### 📊 对比测试结果

**✅ 成功案例：使用Teaclave标准启动脚本**
```bash
# 使用完整的start_qemuv8脚本
export IMG_DIRECTORY='/opt/teaclave/images'
export IMG_NAME='x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory'
export QEMU_HOST_SHARE_DIR='/tmp'
/opt/teaclave/bin/start_qemuv8
```
**结果**: ✅ 完全成功 - 系统完整启动到登录界面，所有功能正常

**❌ 失败案例：使用简化QEMU命令**
```bash
# 简化的qemu启动（在BL1后挂起）
./qemu-system-aarch64 -nodefaults -nographic -serial stdio -smp 2 \
    -machine virt,secure=on,acpi=off,gic-version=3 -cpu cortex-a57 \
    -m 1057 -bios bl1.bin -initrd rootfs.cpio.gz \
    -kernel Image ...
```
**结果**: ❌ 在BL1后系统挂起，无法继续启动

#### 💡 根本原因分析
- **semihosting配置缺失**: ARM semihosting支持必需
- **文件系统配置不完整**: 9p virtio共享文件夹配置复杂
- **内存映射问题**: 需要准确的内存配置参数

### 📋 完整测试状态报告

**测试环境**:
- Docker镜像: `teaclave/teaclave-trustzone-emulator-std-optee-4.5.0-expand-memory:latest`
- TA版本: eth_wallet v1.0 + HelloWorld API扩展
- Host版本: eth_wallet-rs v1.0 + HelloWorld CLI
- 测试时间: 2025-09-29

**测试结果总览**:
1. ✅ TA构建: 100%成功
2. ✅ Host构建: 100%成功
3. ✅ Docker环境: 稳定运行
4. ✅ QEMU启动: 完全正常（使用正确配置）
5. ⚠️ QEMU配置: 需要使用完整的Teaclave启动脚本

## 🎊 最终成功：完整的QEMU OP-TEE标准部署流程

**更新时间: 2025-09-29**

### ✅ 经过验证的成功部署方法

我们最终成功使用**标准9p virtio共享文件夹方法**在QEMU OP-TEE环境中部署和测试了eth_wallet：

#### 🚀 第一步：环境和文件准备
```bash
# 创建专用测试容器
docker run -it --name kms-test-std \
  -v $(pwd):/workspace \
  teaclave/teaclave-trustzone-emulator-std-optee-4.5.0-expand-memory:latest bash

# 准备共享目录并复制文件
docker exec kms-test-std bash -c "
echo '📂 准备共享目录中的TA和host文件...'
mkdir -p /workspace/shared
cp /workspace/kms/ta/target/aarch64-unknown-optee/release/be2dc9a0-02b4-4b33-ba21-9964dbdf1573.ta /workspace/shared/
cp /workspace/kms/host/target/aarch64-unknown-linux-gnu/release/eth_wallet-rs /workspace/shared/
chmod +x /workspace/shared/eth_wallet-rs
echo '✅ 共享文件准备完成'
ls -la /workspace/shared/
"
```

#### 🎯 第二步：使用标准QEMU启动配置
```bash
# 使用完整的Teaclave QEMU启动命令（关键！）
cd /opt/teaclave/images/x86_64-optee-qemuv8-ubuntu-24.04-expand-ta-memory
./qemu-system-aarch64 \
    -nodefaults -nographic \
    -serial stdio -serial file:/tmp/serial.log \
    -smp 2 \
    -machine virt,secure=on,acpi=off,gic-version=3 \
    -cpu cortex-a57 \
    -d unimp -semihosting-config enable=on,target=native \
    -m 1057 \
    -bios bl1.bin \
    -initrd rootfs.cpio.gz \
    -append 'console=ttyAMA0,115200 keep_bootcon root=/dev/vda2' \
    -kernel Image \
    -fsdev local,id=fsdev0,path=/workspace/shared,security_model=none \
    -device virtio-9p-device,fsdev=fsdev0,mount_tag=host \
    -netdev user,id=vmnic,hostfwd=:127.0.0.1:55433-:4433 \
    -device virtio-net-device,netdev=vmnic
```

#### 📱 第三步：系统内标准部署
```bash
# 等待系统启动到登录界面
buildroot login: root

# 创建挂载点并挂载9p共享目录
mkdir shared && mount -t 9p -o trans=virtio host shared

# 进入共享目录验证文件
cd shared/
ls -la
# 应该看到：
# be2dc9a0-02b4-4b33-ba21-9964dbdf1573.ta
# eth_wallet-rs

# 按照README标准流程部署TA
cp be2dc9a0-02b4-4b33-ba21-9964dbdf1573.ta /lib/optee_armtz/

# 验证TA部署成功
ls -la /lib/optee_armtz/be2dc9a0*
```

#### 🧪 第四步：完整功能测试验证
```bash
# 1. 基础help测试
./eth_wallet-rs --help
# ✅ 应显示完整的命令列表包括hello命令

# 2. 关键测试：HelloWorld API（验证TA↔Host通信）
./eth_wallet-rs hello --name 'Standard-Deploy-Test'
# ✅ 预期输出：Hello, Standard-Deploy-Test! This message is from TEE (Trusted Execution Environment).

# 3. 核心钱包功能测试
./eth_wallet-rs create-wallet
# ✅ 预期输出：Wallet ID: [UUID]（例如：2f238bfd-df48-41eb-87eb-b5cd6f588171）

# 4. 稳定性验证
./eth_wallet-rs hello --name 'Final-Stability-Test'
# ✅ 再次成功证明系统稳定

# 5. 系统资源检查
free -h
ps aux | grep -E 'tee|eth' | grep -v grep
```

### 🎉 测试成功结果确认

**完整功能验证通过**：
- ✅ **QEMU OP-TEE环境**：完全正常启动（ARM TrustZone、BL1/BL2/BL31、U-Boot、Linux内核）
- ✅ **OP-TEE驱动**：初始化成功，`/dev/tee*`设备就绪
- ✅ **9p virtio文件系统**：挂载成功，共享文件可访问
- ✅ **TA部署**：成功部署到`/lib/optee_armtz/`
- ✅ **Host应用**：成功执行，CLI界面正常
- ✅ **HelloWorld API**：完美工作，TA↔Host通信正常
- ✅ **钱包创建功能**：正常工作，生成有效UUID
- ✅ **TEE安全环境**：运行稳定，多次调用无问题

### 🔑 关键技术发现

1. **QEMU配置的重要性**：
   - 必须使用完整的Teaclave QEMU启动脚本
   - 简化命令会在BL1后挂起
   - semihosting和正确的文件系统配置是必需的

2. **9p virtio共享文件夹**：
   - 这是标准且推荐的部署方法
   - 无需修改rootfs.cpio.gz
   - 支持动态文件共享

3. **网络配置**：
   - 需要避免端口冲突（使用55433而非54433）
   - hostfwd配置确保网络通信

4. **HelloWorld API扩展**：
   - 成功证明了eth_wallet的可扩展性
   - 演示了完整的TA↔Host API开发流程

### 🚀 结论

**eth_wallet已完全准备好用于真实的KMS API开发！**

- 🔐 TEE安全环境运行稳定
- 💻 Host↔TA通信链路完全正常
- 🛠️ 开发环境配置正确
- 📚 扩展API的方法已验证

**下一步**：可以开始基于这个稳定的基础实现真实的AWS KMS兼容API功能。

---

*最后更新时间: 2025-09-29*