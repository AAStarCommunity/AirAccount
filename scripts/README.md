# KMS开发脚本集

## 📁 脚本列表

### 环境管理
- **`kms-dev-env.sh`** - 主环境管理脚本（STD模式）
  - 拉取Docker镜像
  - 启动/停止容器
  - 构建KMS
  - 容器shell访问

### QEMU运行（三终端）
- **`kms-qemu-terminal2.sh`** - Guest VM监听器（先启动）
- **`kms-qemu-terminal3.sh`** - Secure World日志（第二启动）
- **`kms-qemu-terminal1.sh`** - QEMU启动器（最后启动）

### 快速部署
- **`kms-deploy.sh`** - 增量构建+部署到QEMU
- **`kms-deploy.sh clean`** - 完整重建+部署

## 🚀 快速开始

### 首次使用

```bash
# 1. 初始化环境（一次性）
./scripts/kms-dev-env.sh all

# 2. 启动QEMU（三个终端）
# Terminal 2:
./scripts/kms-qemu-terminal2.sh

# Terminal 3:
./scripts/kms-qemu-terminal3.sh

# Terminal 1:
./scripts/kms-qemu-terminal1.sh

# 3. 在QEMU中运行（Terminal 2）
buildroot login: root
mkdir shared && mount -t 9p -o trans=virtio host shared
cd shared && ./kms --help
```

### 日常开发

```bash
# 修改代码后快速部署
./scripts/kms-deploy.sh

# 在QEMU中测试（Terminal 2）
cd shared
cp *.ta /lib/optee_armtz/
./kms create-wallet
```

## 📖 详细文档

完整工作流程见：[docs/kms-workflow.md](../docs/kms-workflow.md)