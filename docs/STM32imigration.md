# STM32MP157F-DK2 迁移部署终极执行手册

## 0. TODO 列表
1. **启动KMS 服务，对外提供API服务**：`KMS.aastar.io`
2. **增加 passkey 签名验证** 和 **BLS 签名验证**
3. **增加社交恢复的迁入、迁出功能**：页面生成、签名收集和签名验证

---

## 一、现状确认（基于代码审计的客观事实）

### 1.1 当前编译架构

当前 Docker 镜像 `teaclave/teaclave-trustzone-emulator-std-optee-4.5.0-expand-memory` 是一个 **aarch64 (ARMv8, 64位)** 的 QEMU OP-TEE 仿真环境。

`kms/Makefile` (第 20-23 行) 硬编码了目标架构：
```makefile
CROSS_COMPILE_HOST ?= aarch64-linux-gnu-
CROSS_COMPILE_TA   ?= aarch64-linux-gnu-
TARGET_HOST        ?= aarch64-unknown-linux-gnu
TARGET_TA          ?= aarch64-unknown-linux-gnu
```

`kms/ta/Makefile` 和 `kms/host/Makefile` 也各自默认 `aarch64`。

### 1.2 目标硬件

**STM32MP157F-DK2** = 双核 Cortex-A7 = **ARMv7-A (32位)**。

### 1.3 结论

- **代码逻辑一行不改**（C/Rust 业务代码完全兼容 OP-TEE Core API）
- **必须重新交叉编译**：64位 aarch64 的二进制在 32位 ARMv7 板子上会直接报 `Exec format error`
- TA 签名的 `TA_DEV_KIT_DIR` 必须换成 STM32 平台专属的 Export Kit

---

## 二、执行方案（Mac + Docker + STM32 唯一最优路径）

> [!IMPORTANT]
> 您当前只有 Mac，没有 Ubuntu 主机。STM32 板子上自身**没有预装 TA Dev Kit**（已验证 `find / -name "ta_dev_kit.mk"` 返回空）。
> 因此唯一可行方案是：**在 Mac 上用 Docker 跑 Ubuntu 容器做交叉编译，编译产物 SCP 推送到板子**。

### Step 1: 在 Mac 上搭建 Docker 编译环境

```bash
# 在 Mac 终端执行，进入代码目录
cd ~/Dev/mycelium/my-exploration/projects/AirAccount

# 启动 Ubuntu 22.04 容器，将代码挂载进去
docker run -it \
  --name stm32-builder \
  --platform linux/amd64 \
  -v $(pwd):/workspace \
  -w /workspace \
  ubuntu:22.04 /bin/bash
```

> `--platform linux/amd64` 确保在 M 芯片 Mac 上也能正确跑 x86 Ubuntu（ST 的 SDK 只提供 x86 和 arm64 两个版本，用 x86 兼容性最好）。

### Step 2: 在 Docker 内安装交叉编译工具链

进入 Docker 容器后，执行以下命令：
```bash
apt update && apt install -y \
  build-essential git curl wget \
  gcc-arm-linux-gnueabihf g++-arm-linux-gnueabihf \
  binutils-arm-linux-gnueabihf \
  python3 python3-pip python3-pycryptodome \
  pkg-config libssl-dev

# 验证交叉编译器
arm-linux-gnueabihf-gcc --version
# 应显示 gcc (Ubuntu ...) 11.x / 12.x 等
```

### Step 3: 在 Docker 内安装 Rust 及 ARMv7 目标

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# 安装 xargo (TA 编译需要)
cargo install xargo

# 安装 ARM32 编译目标
rustup target add armv7-unknown-linux-gnueabihf

# 安装 nightly (xargo 可能需要)
rustup install nightly
rustup component add rust-src --toolchain nightly
```

### Step 4: 在 Docker 内安装 ST 官方 SDK（获取 TA_DEV_KIT_DIR）

这一步是**核心中的核心**：获取 STM32 平台专属的 OP-TEE TA 编译套件。

```bash
cd /opt

# 方案 A：从 ST 官网下载 Developer Package SDK（推荐）
# 在 Mac 浏览器下载 SDK-x86_64-stm32mp1-openstlinux-6.6-yocto-scarthgap-mpu-v26.02.18.tar.gz
# 然后把它放到 AirAccount 目录下（因为挂载了所以 Docker 里也能看到）
# 在 Docker 内执行:
tar xvf /workspace/SDK-x86_64-stm32mp1-openstlinux-*.tar.gz -C /opt/
chmod +x /opt/stm32mp1-openstlinux-*/sdk/st-image-weston-*.sh
/opt/stm32mp1-openstlinux-*/sdk/st-image-weston-*.sh -d /opt/STM32MP1-SDK

# 激活 SDK 环境 (每次新开终端都要执行)
source /opt/STM32MP1-SDK/environment-setup-cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi

# 验证环境
echo $ARCH        # 应输出: arm
echo $CROSS_COMPILE  # 应输出: arm-ostl-linux-gnueabi-

# 定位 TA_DEV_KIT_DIR（在 SDK sysroots 内搜索）
find /opt/STM32MP1-SDK/sysroots -name "ta_dev_kit.mk" -o -name "export-ta_arm32" 2>/dev/null
# 假设输出:  /opt/STM32MP1-SDK/sysroots/cortexa7t2hf-.../usr/include/optee/export-user_ta/mk/ta_dev_kit.mk
# 则 TA_DEV_KIT_DIR 等于去掉末尾 /mk/ta_dev_kit.mk 的部分
export TA_DEV_KIT_DIR=<find 命令输出的路径去掉末尾 /mk/ta_dev_kit.mk>
```

> **方案 B（如果 SDK 下不到或太大）**：从您的 STM32MP157F-DK2 仓库中找到已编译好的 optee_os 产物 `export-ta_arm32` 目录，拷贝进 Docker。

### Step 5: 交叉编译 TA

```bash
cd /workspace/kms/ta

# 设置环境变量 (如果没有 source SDK，则手动设置)
export CROSS_COMPILE=arm-linux-gnueabihf-
export TARGET=armv7-unknown-linux-gnueabihf
export TA_DEV_KIT_DIR=<Step 4 中 find 到的路径>

# 编译 TA
make clean
make TARGET=$TARGET CROSS_COMPILE=$CROSS_COMPILE

# 验证产物
file kms/ta/target/armv7-unknown-linux-gnueabihf/release/*.ta 2>/dev/null || echo "检查 ta 输出目录"
ls -la target/*/release/
```

### Step 6: 交叉编译 CA (Host/API Server)

```bash
cd /workspace/kms/host

# 配置 Cargo 链接器
mkdir -p /workspace/.cargo
cat > /workspace/.cargo/config.toml << 'EOF'
[target.armv7-unknown-linux-gnueabihf]
linker = "arm-linux-gnueabihf-gcc"
EOF

# 编译 Host
export TARGET_HOST=armv7-unknown-linux-gnueabihf
cargo build --target armv7-unknown-linux-gnueabihf --release

# 验证产物
file target/armv7-unknown-linux-gnueabihf/release/kms-api-server
# 应显示: ELF 32-bit LSB ..., ARM, ...
```

### Step 7: 部署到 STM32 开发板

> **提示**：根据您的网络架构，您的 Mac Mini (IP: `192.168.7.3`) 与 STM32 开发板 (IP: `192.168.7.2`) 通过网线直连，可直接 SSH 通信。

回到 **Mac 终端**（不是 Docker 内）：

```bash
cd ~/Dev/mycelium/my-exploration/projects/AirAccount
BOARD_IP="192.168.7.2"

# 部署 TA
scp kms/ta/target/*/release/*.ta root@$BOARD_IP:/lib/optee_armtz/

# 部署 CA
scp kms/host/target/armv7-unknown-linux-gnueabihf/release/kms-api-server root@$BOARD_IP:/usr/local/bin/kms-server

# 设置权限
ssh root@$BOARD_IP "chmod 444 /lib/optee_armtz/*.ta && chmod +x /usr/local/bin/kms-server"
```

### Step 8: 在开发板上验证

SSH 到板子执行：

```bash
ssh root@$BOARD_IP

# 8.1 检查 TEE 驱动
ls -la /dev/tee0 /dev/teepriv0
ps aux | grep tee-supplicant

# 8.2 手动启动服务测试
kms-server --port 8080 &

# 8.3 端到端验证 (CA -> SMC -> TA 全链路)
curl -s http://127.0.0.1:8080/health

curl -X POST http://127.0.0.1:8080/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}'

# 8.4 如果成功，停掉前台进程
kill %1
```

### Step 9: 配置 7×24 自动运行

```bash
# 在板子上创建 systemd 服务
cat > /etc/systemd/system/kms.service << 'EOF'
[Unit]
Description=AirAccount KMS API & TEE Gateway
After=network.target tee-supplicant.service

[Service]
Type=simple
ExecStart=/usr/local/bin/kms-server --port 8080
Restart=on-failure
RestartSec=5
User=root

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable kms.service
systemctl start kms.service
systemctl status kms.service
```

---

## 三、日常开发迭代工作流

完成首次部署后，日常的代码修改→编译→部署循环如下：

```
Mac 上修改 kms/ 下的 Rust 代码
    ↓
docker start -i stm32-builder   # 重新进入之前的容器
    ↓
cd /workspace/kms && make TARGET=armv7-unknown-linux-gnueabihf CROSS_COMPILE=arm-linux-gnueabihf-
    ↓
回到 Mac，scp 新产物到板子
    ↓
ssh root@板子 "systemctl restart kms"
```

> **提示**：Docker 容器用 `docker start -i stm32-builder` 重新进入，无需每次重装工具链。
