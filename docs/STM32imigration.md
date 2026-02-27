# STM32MP157 TEE 迁移与部署规划专题报告 (STM32imigration)

## 0. TODO 列表 (项目需求规划)
根据最新需求，未来项目需要完成以下任务：
1. **启动KMS 服务，对外提供API服务**：`KMS.aastar.io`
2. **增加 passkey 签名验证** 和 **BLS 签名验证**。
3. **增加社交恢复的迁入、迁出功能**，包括：页面生成、签名收集和签名验证。

---

## 一、 关于 CA (KMS API / 也可以叫 Host) 与 TA 的概念及物理部署位置澄清

您的理解**完全正确**。
*   **KMS API 服务所在的程序 (`kms/host`)，就是通常 OP-TEE 语境下所说的 CA (Client Application)。**
    *   通过我刚刚深入代码 `kms/host/src/api_server.rs` 和 `kms/host/src/ta_client.rs` 的核查，这个 Rust 写的 HTTP 服务，接收到诸如 `CreateKey` 或 `Sign` 等外部网络请求后，会在内部组装请求参数。
    *   接着，封装好的参数会传递给底层基于 `optee_teec` 库封装的 `TaClient`。
    *   `TaClient` 会通过 TEE 驱动发起 **SMC (Secure Monitor Call)** 指令，实质上就是一种跨 CPU 安全状态的上下文切换调用，将包含指令 ID (`proto::Command`) 和 Payload 的内存指针传入到底层系统。
    *   **它的部署位置：** 理所应当运行在 STM32 的 **普通 Linux 操作系统 (Rich Execution Environment, REE)** 上。

*   **TA (Trusted Application, `kms/ta`)**
    *   负责真正的 ECDSA/BLS/Passkey 等高敏感度的密码学运算，并管理位于硬件加解密区的安全存储 (Secure Storage)。
    *   **它的部署位置：** 运行在 STM32 内部隔离的 **OP-TEE 操作系统 (Secure World)** 环境中。

**总结：物理形态上，它们确实是两个编译出来的二进制实体。但业务逻辑上，它们组成了一个不可分割的完整 KMS 服务系统。**

---

## 二、 核心答疑：关于 "必须重新编译" 的事实依据与详细解释

非常抱歉之前的表述方式让您感到了困扰或猜测的意味。您说得对，我应该从代码和配置本身出发，给出确凿的依据，而不是主观推断。

我刚刚重新仔细审阅了仓库中的关键编译配置，特别是 `kms/Makefile` 和 `kms/ta/Makefile`，我们来探讨一下为什么代码确实一行都不用改，但**依然必须进行交叉编译**才能在 STM32MP157 板子上运行。

**事实 1：QEMU 测试环境的真实情况**
您完全正确！您目前使用的 QEMU 环境确实是**真正的 OP-TEE TA**，而不是 Mock。
我查阅了 `docs/deploy-arm-kms.md`，文档明确说明 QEMU OP-TEE 环境提供了真实的 TEE 隔离，仅仅在最初期的算法验证阶段（Phase 1）才使用了 `mock_tee`。既然您已经在 QEMU 内跑通，意味着 `kms/ta` 的 Rust/C 业务代码逻辑已经是完美适配 OP-TEE Core API (`libutee`) 的，因此**底层逻辑代码（C/Rust）确实一行都不需要改**。

**事实 2：为什么要重新编译？（基于 `kms/Makefile` 的硬证据）**
我查阅了您的项目根别 `kms/Makefile` 文件（第 20-23 行）：
```makefile
CROSS_COMPILE_HOST ?= aarch64-linux-gnu-
CROSS_COMPILE_TA ?= aarch64-linux-gnu-
TARGET_HOST ?= aarch64-unknown-linux-gnu
TARGET_TA ?= aarch64-unknown-linux-gnu
```

这四行代码是决定生死的关键所在：
1.  **指令集架构冲突**：上述配置表明，您目前能在 QEMU 跑通的编译产物，其目标架构被硬编码或默认指向了 **`aarch64`（64位 ARMv8 架构）**。QEMU 模拟的正是 64 位的 Cortex-A53 或类似核心。
2.  **STM32MP157F-DK2 的硬件客观限制**：这款真正物理芯片的核心是双核 **Cortex-A7**。Cortex-A7 只有 **32位（ARMv7-A 架构）**。
3.  **结论**：如果您把现有的、能在 QEMU 里跑的 64 位 `.ta` 固件和 `kms-host` 二进制文件直接原封不动拷到只有 32 位物理 CPU 的 STM32 板子上执行，Linux 是绝不可能执行它的（必然引发 `Exec format error` 操作系统级报错）。

**事实 3：TA 签名的加密机制约束**
在 `kms/ta/Makefile` 的第 45 行定义了签名流程：
```makefile
@$(SIGN) --uuid $(UUID) --key $(TA_SIGN_KEY) --in $(OUT_DIR)/stripped_ta --out $(OUT_DIR)/$(UUID).ta
```
打包 `.ta` 文件的脚本极其依赖 `TA_DEV_KIT_DIR`。即便抛开架构 32位不谈，OP-TEE 官方强制要求每个物理硬件平台的 TA 编译，都必须链接对应那块板子系统构建出来的特定的导出套件（Export Kit）。如果用 QEMU 的 Dev Kit 打出的签名，STM32 硬件底层 OP-TEE OS 在加载时验签会不认，也无法分配对应的硬件内存页。

**这就是为什么我说 "必须彻底重新编译" 的根源：逻辑代码完全不变，但目标机器码的架构必须从 64位转换成 32位，且签名材料必须换成 STM32 专属的体系。**

---

## 三、 STM32MP157 编译、部署与持续测试完整标准流程（Migration Plan）

通过仔细研读您提供的 [STM32MP157F-DK2 官方开发文档](https://github.com/jhfnetboy/STM32MP157F-DK2)，针对 AirAccount KMS 结合该硬件的具体情况，我为您规划了以下详尽的开发与部署路径。

官方文档其实为您提供了**两条完全不同的开发路径**，这是之前我们没有涉及到的盲区：

### 路径 A：Mac 用户推荐工作流（板上直接编译 —— 最省事、防错率高）

如果您是通过 Mac 进行开发，官方**强烈建议**直接在开发板上进行本地编译，完美避开 Mac 交叉编译环境配置的巨坑。

1. **环境准备与代码同步**：
   *   Mac 通过 SSH/VNC 连接到装好初始系统（集成了 OP-TEE 和 gcc等工具链）的 STM32MP157 开发板 (`root@<board-ip>`)。
   *   在板子上直接 Clone 代码：`git clone https://github.com/AAStarCommunity/AirAccount.git`，并切换到 `KMS-stm32` 分支。
2. **在板上直接编译 TA (`kms/ta`)**：
   因为是在 ARMv7 开发板本尊上直接编译给自己用，所以**不需要**设置 `CROSS_COMPILE`，使用的是板上的原生 GCC！
   ```bash
   cd ~/AirAccount/kms/ta
   # 指向开发板上预装的 TA dev kit，通常在：
   export TA_DEV_KIT_DIR=/usr/lib/optee_armtz  # (具体路径以板上实际安装为准)
   make
   ```
3. **在板上编译 CA/Host (`kms/host`)**：
    由于是直接在目标板的 Linux 系统上编译：
    ```bash
    cd ~/AirAccount/kms/host
    cargo build --release  # 无需设置 --target 交叉编译目标
    ```
4. **原地部署部署与测试**：
    ```bash
    cp ~/AirAccount/kms/ta/*.ta /lib/optee_armtz/
    cp ~/AirAccount/kms/host/target/release/kms-server /usr/local/bin/
    ```

### 路径 B：Ubuntu 分布式开发工作流（宿主机交叉编译 —— 适合重度开发）

如果您在 Ubuntu 或 Linux 虚拟机上进行重度开发，编译速度更快的方式是传统的交叉编译，这也是大部分底层嵌入式工程师采用的做法：

1. **使用官方脚本一键部署基础交叉工具链**：
   在 Ubuntu 宿主机上，拉取并运行您仓库中的一键环境脚本，这会配置好 `arm-linux-gnueabihf-gcc` 及 Yocto 依赖：
   ```bash
   git clone https://github.com/jhfnetboy/STM32MP157F-DK2.git
   cd STM32MP157F-DK2
   ./scripts/setup-ubuntu-dev-env.sh
   ```
2. **安装 STM32 MPU 官方 Developer Package SDK (获取 `TA_DEV_KIT_DIR`)**：
   *这一步是为了获取编译 TA 必须用到的特定板子的签名秘钥和头文件环境 (`TA_DEV_KIT_DIR`)*
   *   前往 ST 官网下载对应的 SDK 压缩包 (例如: `SDK-x86_64-stm32mp1-openstlinux-6.6-yocto-scarthgap-mpu-v26.02.18.tar.gz`)。
   *   解压并执行里边的 `.sh` 安装脚本，指定安装目标路径：
       ```bash
       tar xvf SDK-x86_64-stm32mp1-openstlinux-*.tar.gz
       chmod +x stm32mp1-openstlinux-*/sdk/st-image-weston-openstlinux-*.sh
       ./stm32mp1-openstlinux-*/sdk/st-image-weston-openstlinux-*.sh -d ~/STM32MPU_workspace/Developer-Package/SDK
       ```
   *   每次打开新终端准备编译前，**必须 source 这个环境变量**：
       ```bash
       cd ~/STM32MPU_workspace/Developer-Package
       source SDK/environment-setup-cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
       ```
       *此时系统已装载交叉编译器。而您编译 TA 苦苦寻找的 `TA_DEV_KIT_DIR`，它就位于您刚刚解压出来的 SDK 根目录下的 sysroots 目标设备架构文件夹内！*
       通常路径格式为：`export TA_DEV_KIT_DIR=$OECORE_TARGET_SYSROOT/usr/include/optee/export-user_ta` 或类似位置。您可以通过 `find ~/STM32MPU_workspace/Developer-Package/SDK/sysroots -name "export-ta_arm32"` 精确找寻。

3. **在宿主机交叉编译 TA 与 CA**：
   与我们在"核心答疑"中讨论的一致：
   ```bash
   # 编译 TA
   cd AirAccount/kms/ta
   export CROSS_COMPILE=arm-linux-gnueabihf-
   export TA_DEV_KIT_DIR=~/optee-stm32mp1/optee_os/out/arm-plat-stm32mp1/export-ta_arm32  # 示例路径
   make
   
   # 编译 CA
   cd ../host
   rustup target add armv7-unknown-linux-gnueabihf
   cargo build --target armv7-unknown-linux-gnueabihf --release
   ```
3. **通过 SCP 推送到开发板**：
   将生成的 `.ta` 和 `kms-server` 推送到板子的 `/lib/optee_armtz/` 和 `/usr/bin/` 中。

---

### Phase 3: 黑盒连通性与高可用性测试 (Execution & Availability)

代码的 SMC 通道能否正常工作取决于硬件 TEE 驱动的支持状态，无论采用哪条路径，最终部署后的验证环节都是一致的：

**Step 3.1: 基础驱动存活测试**
在 STM32 的终端中执行：
```bash
# 验证 tee-supplicant 守护进程是否在后台运行 (它是 TA 访问普通世界存储的关键)
ps aux | grep tee-supplicant

# 验证内核驱动是否暴露了安全的硬件字符设备
ls -la /dev/tee0
ls -la /dev/teepriv0
```

**Step 3.2: 本地 API 集成存活测试**
开启一个前台终端运行服务，观察启动日志：
```bash
kms-server --port 8080
```
另开一个 SSH 会话进行全量端到端验证（这步至关重要，代表了 CA -> SMC -> TA 的全链路打通）：
```bash
# 1. 探针测活
curl -s http://127.0.0.1:8080/health

# 2. 发起重度交互（穿透到真实硬件晶圆生成 ECDSA）
curl -X POST http://127.0.0.1:8080/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}'
```

**Step 3.3: 7x24 小时无人值守部署 (Production Daemon)**
当上述手动测试无误后，必须配置 Systemd 以实现程序的异常崩溃拉起和随终端断电开机自启。
1. `nano /etc/systemd/system/kms.service`
```ini
[Unit]
Description=AirAccount KMS API & TEE Gateway
After=network.target tee-supplicant.service

[Service]
Type=simple
ExecStart=/usr/local/bin/kms-server --port 8080
Restart=on-failure
RestartSec=5
# 确保以具有 /dev/tee0 读写权限的用户组启动 (如果是非 root 跑，需把用户加入 tee 用户组)
User=root 

[Install]
WantedBy=multi-user.target
```
2. 执行激活：
```bash
systemctl daemon-reload
systemctl enable kms.service
systemctl start kms.service
systemctl status kms.service
```

通过这一套流程设计，我们就能安全、稳定地将 AirAccount KMS 在这块支持 TrustZone 的 STM32MP1 开发板上完成真实的工业级硬件落地。
