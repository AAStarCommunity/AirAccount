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

### Phase 1: 准备交叉编译上下文与产物构建

1.  **获取对应环境的库与 Dev Kit**:
    确保拥有针对 STM32 架构编译好的 OP-TEE Client 库 (`libteec.so`) 以及 OP-TEE OS 给 TA 用的 `export-ta_arm32` 目录。
2.  **编译 TA (`kms/ta`)**:
    ```bash
    cd kms/ta
    # 指向 STM32 的交叉编译器
    export CROSS_COMPILE=arm-none-linux-gnueabihf-
    # 指向 STM32 平台编译出来的 TA dev kit
    export TA_DEV_KIT_DIR=/path/to/stm32/optee_os/out/arm-plat-stm32mp1/export-ta_arm32
    make
    ```
    *产物验证*：输出一个带有类似 `4319f351-0b24-4097-b659-80ee4f824cdd.ta` 的文件。可通过 `file [UUID].ta` 验证是否为 32-bit ARM 数据格式。
3.  **编译 CA/Host API (`kms/host`)**:
    在 Rust 环境中安装对应 target：
    ```bash
    rustup target add armv7-unknown-linux-gnueabihf
    # 在 ~/.cargo/config 中配置对应架构的链接器，指向 STM32 的 GCC 工具链
    cd kms/host
    cargo build --target armv7-unknown-linux-gnueabihf --release 
    ```
    *产物验证*：生成可执行文件 `target/armv7-unknown-linux-gnueabihf/release/kms-server`。

### Phase 2: 实施文件转移与硬件部署

将板卡连接上网络或通过 UART/SCP，将两个编译好的产物推送入 STM32 的 Linux (Yocto/Buildroot/Debian系统)：
1.  **部署 TA**:
    将生成的 `.ta` 文件拷贝至物理板的标准查找位置：`/lib/optee_armtz/` 或 `/usr/lib/optee_armtz/`。
2.  **部署 CA**:
    将 `kms-server` 拷贝至 `/usr/local/bin/`，赋予 `chmod +x`。

*(前提假设是您当前的 STM32MP157 硬件的 SD 卡底层固件中已经成功集成了 TF-A, OP-TEE OS 及 Linux Kernel 中的 tee 驱动，并且后台驻留了进程 `tee-supplicant`。)*

### Phase 3: 黑盒连通性与高可用性测试 (Execution & Availability)

代码的 SMC 通道能否正常工作取决于硬件 TEE 驱动的支持状态，部署后的测试流程应当如下：

**Step 3.1: 基础驱动存活测试**
在 STM32 的终端中执行：
```bash
# 验证 tee-supplicant 进程是否存在
ps aux | grep tee-supplicant

# 验证内核驱动是否暴露了安全的字符设备
ls -la /dev/tee0
ls -la /dev/teepriv0
```

**Step 3.2: 本地 API 集成存活测试**
开启一个前台终端运行服务看输出（不要关终端）：
```bash
./kms-server --port 8080
```
另开一个 SSH 会话进行全量端到端验证（这步至关重要，它验证了 Linux CA 通过物理内核态 TEE Client 驱动呼叫处于芯片内存隔离区里面的 TA 的能力）：
```bash
# 1. 查询健康度（不调TA）
curl -s http://127.0.0.1:8080/health

# 2. 发起重度交互（穿透到TA生成 ECDSA）
curl -X POST http://127.0.0.1:8080/ \
  -H "Content-Type: application/json" \
  -H "X-Amz-Target: TrentService.CreateKey" \
  -d '{"KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1"}'
```

**Step 3.3: 7x24 小时无人值守部署 (Systemd 托管模式)**
当上述测试无误后，立刻终止前台进程。为服务编写 Linux `systemd` 配置以实现异常崩溃拉起、随开机自动启动，这是确保长期无故障调用的根本机制：
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
# 确保以具有 /dev/tee0 读写权限的用户组启动
User=root 

[Install]
WantedBy=multi-user.target
```
2. 执行激活并检查：
```bash
systemctl daemon-reload
systemctl enable kms.service
systemctl start kms.service
systemctl status kms.service
```

通过这一套包含重新针对 STM32v7 进行架构交叉编译、正确放缩文件层级并在硬件层面进行进程监控的方案，您之前在 QEMU 模拟环境中稳定运行的 `kms/host` 和 `kms/ta` 可在真机层面获得企业级的部署稳定性和服务生命周期保障。
