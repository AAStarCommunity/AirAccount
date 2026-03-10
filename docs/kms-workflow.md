# KMS开发工作流程

## 快速开始

### 一次性环境初始化

```bash
# 在项目根目录执行
./scripts/kms-dev-env.sh all
```

这会自动完成：
1. ✅ 拉取STD模式Docker镜像
2. ✅ 启动开发容器（std/aarch64模式）
3. ✅ 构建KMS项目
4. ✅ 同步构建产物到QEMU共享目录

---

## 三个核心动作

### 动作1：初始化环境并启动QEMU

**目的：** 启动开发容器和QEMU虚拟机，准备运行环境

#### 1.1 环境初始化（首次或重置时）

```bash
./scripts/kms-dev-env.sh all
```

#### 1.2 启动QEMU - 三终端模式

**⚠️ 重要：启动顺序必须是 2 → 3 → 1**

**Terminal 2** - Guest VM监听器（先启动）
```bash
./scripts/kms-qemu-terminal2.sh
```
等待显示：`Listening on port 54320...`

**Terminal 3** - Secure World日志监听器（第二启动）
```bash
./scripts/kms-qemu-terminal3.sh
```
等待显示：`Listening on port 54321...`

**Terminal 1** - QEMU启动器（最后启动）
```bash
./scripts/kms-qemu-terminal1.sh
```

#### 1.3 登录QEMU Guest VM

QEMU启动后，在Terminal 2中：
```bash
buildroot login: root
# mkdir shared && mount -t 9p -o trans=virtio host shared
# cd shared
```

---

### 动作2：KMS代码开发

**目的：** 在宿主机开发KMS功能

#### 开发目录结构
```
kms/
├── host/
│   ├── src/
│   │   ├── main.rs      # CLI入口
│   │   ├── api.rs       # 🎯 待添加：API服务模块
│   │   └── ta_client.rs # 🎯 待添加：TA通信封装
│   └── Cargo.toml
├── ta/
│   └── src/
│       └── lib.rs       # TA实现（钱包逻辑）
└── proto/
    └── src/
        └── lib.rs       # CA/TA共享数据结构
```

#### 推荐开发流程

1. **添加新Command**（在proto/src/lib.rs）
   ```rust
   pub enum Command {
       CreateWallet,
       // ... 现有命令
       YourNewCommand, // 新增
   }
   ```

2. **实现TA端逻辑**（在ta/src/lib.rs）
   ```rust
   Command::YourNewCommand => {
       // TA实现
   }
   ```

3. **实现Host端调用**（在host/src/main.rs）
   ```rust
   // CLI处理或API接口
   ```

4. **快速部署测试** → 见动作3

---

### 动作3：构建并部署到QEMU

**目的：** 快速迭代 - 构建新版本并在QEMU中测试

#### 3.1 快速部署（增量构建）

```bash
./scripts/kms-deploy.sh
```

- 增量编译（只编译修改部分，快速）
- 自动同步到QEMU共享目录
- 显示部署后的运行指令

#### 3.2 完整重建（清理后构建）

```bash
./scripts/kms-deploy.sh clean
```

- 清理所有旧构建
- 完整重新编译
- 适用于依赖变更或首次构建

#### 3.3 在QEMU中运行

部署完成后，在**Terminal 2 (Guest VM)** 中：

```bash
# 已在 shared 目录中

# 更新TA（每次TA修改后必须）
cp *.ta /lib/optee_armtz/

# 运行KMS
./kms --help                    # 查看帮助
./kms create-wallet             # 创建钱包
./kms derive-address -w <UUID>  # 派生地址
./kms sign-transaction ...      # 签名交易
```

---

## 常用命令速查

### 环境管理

```bash
# 查看容器状态
./scripts/kms-dev-env.sh status

# 停止容器
./scripts/kms-dev-env.sh stop

# 重启容器
./scripts/kms-dev-env.sh restart

# 进入容器调试
./scripts/kms-dev-env.sh shell

# 仅构建（不部署）
./scripts/kms-dev-env.sh build
```

### 调试技巧

**查看TA日志（Terminal 3）：**
- TA的所有打印会实时显示在这里
- 包括 `println!`, `panic` 等

**查看Guest VM交互（Terminal 2）：**
- CA的输出和命令执行结果

**手动构建（容器内）：**
```bash
docker exec -it teaclave_dev_env bash -l
cd /root/teaclave_sdk_src/kms
make clean && make
```

---

## 典型开发循环

```
1. 修改代码（宿主机 VSCode/IDE）
     ↓
2. ./scripts/kms-deploy.sh  （快速构建+部署）
     ↓
3. 在QEMU中测试（Terminal 2）
     ↓
4. 查看TA日志（Terminal 3）
     ↓
5. 重复 1-4
```

**预计时间：**
- 增量构建：~10-30秒
- 完整构建：~2-3分钟

---

## 问题排查

### Q1: 容器启动失败
```bash
# 检查是否有旧容器
docker ps -a | grep teaclave_dev_env
docker rm -f teaclave_dev_env  # 强制删除

# 重新启动
./scripts/kms-dev-env.sh start
```

### Q2: QEMU连接失败（Connection refused）
**原因：** Terminal 1 在 Terminal 2/3 之前启动

**解决：**
1. 在Terminal 1按 Ctrl+C 停止QEMU
2. 确保Terminal 2和3都在运行
3. 重新运行Terminal 1脚本

### Q3: 构建失败 - 找不到依赖
```bash
# 验证环境配置
docker exec teaclave_dev_env bash -l -c "switch_config --status"
# 应显示: TA: std/aarch64

# 验证rust符号链接
docker exec teaclave_dev_env bash -l -c "ls -la /root/teaclave_sdk_src/rust"
# 应显示: rust -> /opt/teaclave/std

# 重新创建符号链接
docker exec teaclave_dev_env bash -l -c "cd /root/teaclave_sdk_src && rm -rf rust && ln -s /opt/teaclave/std rust"
```

### Q4: TA无法加载（UUID not found）
```bash
# 在Guest VM中验证TA文件
ls -la /lib/optee_armtz/*.ta

# 重新复制（确保UUID正确）
cd shared
cp de29f316-8794-4f88-bbab-033748c7ce37.ta /lib/optee_armtz/
```

---

## 下一步：添加API服务

根据之前讨论的架构，API服务模块规划：

**kms/host/src/api.rs**
```rust
// JSON-RPC API服务
// - HTTP Server (actix-web/axum)
// - AWS KMS兼容接口
// - 调用ta_client与TA通信
```

**kms/host/src/ta_client.rs**
```rust
// TA通信封装
// - Context管理
// - Session管理
// - Command封装
```

这部分将在动作2中实现。

---

## 相关文档

- [eth_wallet README](../third_party/teaclave-trustzone-sdk/projects/web3/eth_wallet/README.md) - 原始项目文档
- [OP-TEE Storage Analysis](./optee-storage-analysis.md) - 存储安全分析
- [KMS API Architecture](./kms-api-architecture-discussion.md) - API架构讨论
- [Changes Log](./Changes.md) - 完整开发日志