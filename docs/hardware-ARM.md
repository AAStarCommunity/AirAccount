# ARM TEE 开发板推荐 - 替代 QEMU 模拟环境

基于 Apache Teaclave TrustZone SDK 和 OP-TEE 官方支持的开发板推荐指南。

---

## 🎯 推荐总结

| 开发板 | 推荐度 | 价格 | 供货状态 | 生产可用 | 适用场景 |
|--------|--------|------|----------|----------|----------|
| **STM32MP157F-DK2** | ⭐⭐⭐⭐⭐ | $100-200 | ✅ 稳定供货 | ✅ 是 | 开发+生产 |
| **HiKey 960** | ⭐⭐⭐ | $200-300 | ⚠️ 缺货 | ✅ 是 | 高性能开发 |
| **Raspberry Pi 3/4** | ⭐ | $35-75 | ✅ 容易购买 | ❌ 否 | 仅供学习 |

---

## ✅ 强烈推荐：STM32MP157F-DK2 (STMicroelectronics)

### 推荐理由

1. ✅ **生产级别芯片** - STM32MP157C/F 截至 2024年9月处于全面生产状态
2. ✅ **完整 TrustZone 支持** - 真实的硬件安全隔离，支持 OP-TEE 作为默认 BL32
3. ✅ **容易购买** - 多个官方渠道可购买（ST官方商店、Digi-Key、Mouser、Farnell、Newark）
4. ✅ **丰富的外设** - LCD触摸屏、WiFi、蓝牙、eMMC、SD卡
5. ✅ **官方文档完善** - Trusted Firmware-A 和 OP-TEE 官方文档支持

### 技术规格

**处理器：**
- **CPU**: Dual Cortex-A7 @ 800MHz (Secure World 运行 OP-TEE)
- **MCU**: Cortex-M4 @ 209MHz (可选实时任务)
- **架构**: ARMv7-A with TrustZone

**内存与存储：**
- **RAM**: 512MB DDR3L
- **eMMC**: 4GB (支持 RPMB 安全分区)
- **存储扩展**: microSD 卡槽

**外设：**
- **显示**: 4.0" WVGA (480x800) LCD 电容触摸屏
- **网络**: WiFi 802.11 b/g/n, Bluetooth 4.1 LE
- **音频**: 音频编解码器，立体声耳机，扬声器
- **摄像头**: DCMI 接口
- **USB**: USB Type-C (OTG), USB Type-A (Host)
- **以太网**: 千兆以太网 (STM32MP157F-EV1)
- **调试**: ST-LINK/V2-1 调试器

### 安全特性

**TrustZone 支持：**
- ARM TrustZone 硬件隔离（Cortex-A7）
- 安全/非安全内存分离
- 安全外设访问控制
- 可配置安全 RAM：256kB ~ 640kB

**安全存储：**
- eMMC RPMB 分区（Replay Protected Memory Block）
- 适合存储密钥、证书等敏感数据
- 硬件防回滚攻击

**安全启动：**
- Trusted Firmware-A (TF-A)
- OP-TEE 作为 Secure Monitor
- 支持 EFI 安全变量保护

**SCMI 安全服务：**
- 非安全世界通过 SCMI 协议访问受保护资源
- 增强的芯片安全加固
- 细粒度的安全策略配置

### OP-TEE 支持

**官方支持的 STM32MP1 开发板：**
1. STM32MP135F-DK
2. STM32MP157A-DK1, STM32MP157D-DK1
3. **STM32MP157C-DK2**, **STM32MP157F-DK2** ⭐
4. STM32MP157C-EV1, STM32MP157F-EV1

**启动方式：**
- ✅ SD 卡启动（默认，OP-TEE 官方支持）
- ⚠️ eMMC 启动（硬件支持但 OP-TEE 分发版仅支持 SD 卡）
- ⚠️ NOR Flash / NAND Flash (仅 EV1 变体)

**配置选项：**
- **Non-SCMI 变体**: 无芯片根安全加固
- **SCMI 变体**: 启用增强安全，通过 OP-TEE SCMI 服务访问受保护资源

### 购买信息

**官方渠道：**
- **ST 官方商店**: https://estore.st.com/en/stm32mp157f-dk2-cpn.html
- **Digi-Key**: https://www.digikey.com/ (搜索 STM32MP157F-DK2)
- **Mouser**: https://www.mouser.com/
- **Farnell**: https://uk.farnell.com/
- **Newark**: https://www.newark.com/

**价格：** 约 $100-200 USD

**型号选择：**
- **STM32MP157F-DK2** (推荐) - 带 LCD 触摸屏、WiFi、蓝牙
- **STM32MP157C-DK2** - 基础版本，功能较少
- **STM32MP157F-EV1** - 评估板，带千兆以太网、更多接口

### 开发资源

**官方文档：**
- OP-TEE 文档: https://optee.readthedocs.io/en/latest/building/devices/stm32mp1.html
- Trusted Firmware-A: https://trustedfirmware-a.readthedocs.io/en/latest/plat/st/stm32mp1.html
- ST Wiki: https://wiki.st.com/stm32mpu/

**社区支持：**
- ST Community: https://community.st.com/
- OP-TEE GitHub: https://github.com/OP-TEE/optee_os

---

## ⚠️ 备选方案 1：HiKey 960 (96Boards)

### 优势

- ✅ 性能强劲 - Kirin 960 八核（4x Cortex-A73 + 4x Cortex-A53）
- ✅ 大内存 - 3GB/4GB LPDDR4
- ✅ OP-TEE 官方支持
- ✅ 适合 AOSP (Android Open Source Project) 开发
- ✅ 96Boards 标准接口

### 劣势

- ❌ **供货困难** - 2024年多数渠道缺货
- ❌ 价格较高
- ⚠️ 可能需要从二手市场购买
- ⚠️ 社区活跃度下降

### 技术规格

**处理器：**
- **CPU**: HiSilicon Kirin 960 Octa-core
  - 4x ARM Cortex-A73 @ 2.4GHz
  - 4x ARM Cortex-A53 @ 1.8GHz
- **GPU**: Mali G71 MP8

**内存与存储：**
- **RAM**: 3GB/4GB LPDDR4
- **存储**: 32GB UFS 2.0
- **扩展**: microSD 卡槽

**外设：**
- HDMI 1.2a (4K)
- USB 3.0 Type-C (OTG)
- USB 2.0 Type-A (Host)
- WiFi 802.11 a/b/g/n/ac, Bluetooth 4.1
- GPS

### 购买信息

**可能的渠道：**
- 96Boards 官网: https://www.96boards.org/product/hikey960/
- Seeed Studio: https://www.seeedstudio.com/ (库存不稳定)
- Amazon (偶尔有货)
- eBay 二手市场

**价格：** $200-300 USD (如果有货)

**库存警告：** 自 2019 年起多个用户报告全球缺货，购买前请确认库存

### OP-TEE 支持

- ✅ OP-TEE 官方支持
- ✅ Trusted Firmware-A 支持
- ✅ 96Boards 标准 UART 调试接口
- 📚 文档: https://optee.readthedocs.io/en/latest/building/devices/hikey960.html

---

## ❌ 不推荐：Raspberry Pi 3/4

### 致命缺陷

**OP-TEE 官方警告：**
> "This port of Trusted Firmware A and OP-TEE to Raspberry Pi 3 **IS NOT SECURE!**"

**硬件限制：**
1. ❌ **缺乏真实安全硬件** - AXI 总线的安全/非安全标志未连接到内存控制器
2. ❌ **无内存隔离** - 虽然 CPU 支持 TrustZone，但无法实现安全内存隔离
3. ❌ **无安全启动** - 缺少安全启动所需的硬件机制
4. ❌ **无安全外设** - 外设无法分配到安全世界

### 适用场景

**仅适合：**
- ✅ 学习 TrustZone 基本概念
- ✅ OP-TEE API 开发练习
- ✅ 教育演示

**绝对不可用于：**
- ❌ 任何安全敏感应用
- ❌ 生产环境
- ❌ 密钥管理系统（如我们的 KMS 项目）
- ❌ 真实的安全存储

### 技术说明

**问题根源：**
- Raspberry Pi 的 BCM2837/BCM2711 芯片虽然使用 ARM Cortex-A53/A72，这些核心支持 TrustZone
- 但 Broadcom 的 SoC 设计没有将 AMBA AXI 总线的安全位连接到内存控制器
- 这意味着即使 OP-TEE 运行在 Secure World，也无法阻止 Normal World 访问"安全"内存

**引用 OP-TEE 文档：**
> "The mechanisms and hardware required to implement secure boot, memory, peripherals or other secure functions are not available."

### 参考资源

- OP-TEE RPi3 文档: https://optee.readthedocs.io/en/latest/building/devices/rpi3.html
- 技术讨论: https://github.com/OP-TEE/optee_os/issues/3205
- 安全分析论文: "Attacking TrustZone on devices lacking memory protection"

---

## 📋 其他支持的开发板（参考）

根据 OP-TEE 官方文档，以下开发板也受支持：

### 1. Texas Instruments (TI) 平台

**支持的 SoC：**
- AM335x, AM437x (Cortex-A9)
- AM57xx, DRA7xx (Cortex-A15)
- AM65x (Cortex-A53)

**特点：**
- ✅ 工业级可靠性
- ✅ 长期供货保证
- ✅ 丰富的实时处理能力（PRU）
- ⚠️ 文档相对较少
- ⚠️ 开发难度较高

**适用场景：** 工业控制、汽车电子

### 2. Xilinx Zynq UltraScale+ MPSoC

**特点：**
- ✅ FPGA + ARM Cortex-A53 (4核)
- ✅ 实时处理器 Cortex-R5 (2核)
- ✅ 可编程逻辑 (FPGA)
- ❌ 价格昂贵（开发板 $500+）
- ⚠️ 学习曲线陡峭

**适用场景：** 高性能计算、自定义硬件加速、航空航天

### 3. ROCK Pi 4 (Radxa)

**特点：**
- ✅ RK3399 六核（2x A72 + 4x A53）
- ✅ 容易购买
- ✅ 价格适中（$50-100）
- ⚠️ OP-TEE 支持由社区维护
- ⚠️ 官方文档有限

**适用场景：** 社区项目、原型开发

### 4. STM32MP135F-DK

**特点：**
- ✅ 单核 Cortex-A7 @ 1GHz + Cortex-M33
- ✅ STM32MP1 系列入门级
- ✅ 价格较低
- ⚠️ 性能弱于 STM32MP157

**适用场景：** 低功耗 IoT、成本敏感项目

---

## 🎯 针对 KMS 钱包应用的最终推荐

### 为什么选择 STM32MP157F-DK2？

**1. 真实的生产级 TrustZone**
- 与当前 QEMU 环境完全兼容
- Cortex-A7 TrustZone 硬件隔离
- 安全内存控制器

**2. 容易购买与供货稳定**
- 2024年持续供货
- 多个官方分销渠道
- 芯片处于全面生产状态（截至 2024年9月）

**3. 完整的 Teaclave SDK 支持**
- 基于 OP-TEE 3.x+
- 无需修改 TA 代码架构
- 仅需重新编译到新目标

**4. 丰富的安全特性**
- **eMMC RPMB**: 完美替代当前的不安全存储
  - 硬件防回滚攻击
  - 加密认证写入
  - 适合存储钱包密钥
- **安全启动**: 保护启动链完整性
- **SCMI 安全服务**: 细粒度权限控制

**5. 价格合理**
- $100-200 性价比高
- 包含 LCD、WiFi、BLE 等外设
- 无需额外购买调试器（板载 ST-LINK）

**6. 可扩展到生产**
- STM32MP157 芯片可用于量产
- 从 DK2 开发板到定制主板路径清晰
- ST 提供完整的生产工具链

---

## 📝 从 QEMU 迁移到 STM32MP157F-DK2

### 迁移路径

#### 1. 编译环境变更

**当前 (QEMU):**
```bash
# TA 目标
TARGET_TA: aarch64-unknown-optee

# Host 目标
TARGET_HOST: aarch64-unknown-linux-gnu
```

**迁移后 (STM32MP1):**
```bash
# TA 目标 (32位 ARMv7-A)
TARGET_TA: arm-unknown-optee

# Host 目标 (32位)
TARGET_HOST: arm-unknown-linux-gnueabihf
```

**重要变化：**
- QEMU 使用 64位 ARMv8-A (aarch64)
- STM32MP157 使用 32位 ARMv7-A (arm)
- 需要安装 32位交叉编译工具链

#### 2. 部署方式变更

**当前 (QEMU):**
```bash
# 共享目录部署
/opt/teaclave/shared/
  ├── kms-api-server (Host)
  └── kms-wallets-backup/

# TA 部署
/lib/optee_armtz/
  └── <TA-UUID>.ta
```

**迁移后 (STM32MP1):**
```bash
# SD 卡启动分区
/boot/
  ├── Image (Linux 内核)
  ├── stm32mp157f-dk2.dtb (设备树)
  └── ...

# Root 文件系统
/usr/bin/kms-api-server
/lib/optee_armtz/<TA-UUID>.ta
/root/kms-wallets-backup/
```

**部署流程：**
```bash
# 1. 编译 OP-TEE 完整系统
cd optee-stm32mp1-build
make all

# 2. 烧录 SD 卡
sudo dd if=out/stm32mp1/sdcard.img of=/dev/sdX bs=4M

# 3. 挂载 SD 卡并复制应用
mount /dev/sdX2 /mnt
cp kms-api-server /mnt/usr/bin/
cp <TA-UUID>.ta /mnt/lib/optee_armtz/
umount /mnt
```

#### 3. 调试方式变更

**当前 (QEMU):**
```bash
# socat TCP 连接
socat TCP:localhost:54320

# 或串口
/dev/pts/X
```

**迁移后 (STM32MP1):**
```bash
# UART 串口连接 (UART4)
# 使用 96Boards UART Serial 适配器
screen /dev/ttyUSB0 115200

# 或 minicom
minicom -D /dev/ttyUSB0 -b 115200
```

**串口参数：**
- 波特率: 115200
- 数据位: 8
- 停止位: 1
- 校验: None
- 流控: None

#### 4. 存储架构变更

**当前 (QEMU):**
```rust
// 不安全的文件系统存储
// /root/shared/kms-wallets-backup/*.json
```

**迁移后 (STM32MP1 - 使用 RPMB):**
```rust
// Trusted Application 内部使用 OP-TEE 安全存储 API
// 数据存储在 eMMC RPMB 分区

use optee_utee::*;

// 创建安全对象
let mut object = persistent_object::PersistentObject::create(
    StorageId::Private,
    "wallet_keystore",
    Flags::DATA_ONLY | Flags::ACCESS_WRITE,
    None,
    &wallet_data,
)?;

// RPMB 存储特性：
// - 硬件加密认证
// - 防回滚攻击
// - 仅 TA 可访问
// - 掉电保持
```

**存储容量规划：**
- RPMB 分区: 4MB (足够存储数千个钱包密钥)
- 普通存储: 4GB eMMC + SD 卡扩展

#### 5. 网络配置

**当前 (QEMU):**
```bash
# 端口转发
Host (macOS) :3000 -> Docker :3000 -> QEMU Guest :3000
```

**迁移后 (STM32MP1):**
```bash
# 选项 1: 以太网 (需要 USB-to-Ethernet 或 STM32MP157F-EV1)
ifconfig eth0 192.168.1.100 netmask 255.255.255.0
route add default gw 192.168.1.1

# 选项 2: WiFi (DK2 板载)
wpa_supplicant -B -i wlan0 -c /etc/wpa_supplicant.conf
dhclient wlan0

# 选项 3: USB 网络 (USB-OTG)
# 通过 USB Gadget 模式连接到 PC
```

#### 6. 自动启动配置

**创建 systemd 服务：**
```ini
# /etc/systemd/system/kms-api-server.service
[Unit]
Description=KMS API Server
After=network.target tee-supplicant.service

[Service]
Type=simple
ExecStart=/usr/bin/kms-api-server
Restart=always
RestartSec=5
User=root
Environment="LD_LIBRARY_PATH=/usr/lib/optee_armtz"

[Install]
WantedBy=multi-user.target
```

**启用服务：**
```bash
systemctl enable kms-api-server
systemctl start kms-api-server
```

### 代码修改需求

**TA 代码 (kms/ta/src/main.rs):**
```rust
// ✅ 无需修改
// Teaclave TrustZone SDK 抽象层隐藏了平台差异
// 仅需重新编译到 arm-unknown-optee 目标
```

**Host 代码 (kms/host/src/main.rs):**
```rust
// ✅ 基本无需修改
// 可能需要调整的部分：

// 1. 备份目录路径
// 从: /root/shared/kms-wallets-backup/
// 到: /var/lib/kms/backup/

// 2. 监听地址（可选）
// 从: 0.0.0.0:3000
// 到: 根据网络配置调整
```

**Cargo.toml:**
```toml
# 添加条件编译特性（可选）
[features]
default = []
qemu = []
stm32mp1 = []

# 根据目标平台选择依赖
[target.'cfg(target_arch = "aarch64")'.dependencies]
# QEMU 特定依赖

[target.'cfg(target_arch = "arm")'.dependencies]
# STM32MP1 特定依赖
```

### 性能对比

| 指标 | QEMU (ARMv8-A) | STM32MP157F-DK2 |
|------|----------------|-----------------|
| CPU | 模拟 Cortex-A57 | 真实 Cortex-A7 @800MHz (双核) |
| 架构 | 64位 (aarch64) | 32位 (armv7-a) |
| TrustZone | 软件模拟 | 硬件实现 ✅ |
| 安全存储 | 文件系统 (不安全) | eMMC RPMB ✅ |
| 启动时间 | ~30秒 | ~10秒 |
| 签名性能 | 取决于 Host CPU | ~5-10ms/签名 |
| 功耗 | N/A | ~1-2W (待机) |

**预期性能：**
- CreateKey: 50-100ms
- Sign: 5-10ms
- ExportKey: 10-20ms
- ListKeys: <5ms

### 开发工作量估算

**工作项：**
1. ☑️ 购买开发板：1-2周（等待物流）
2. ☑️ 搭建编译环境：1-2天
3. ☑️ 编译 OP-TEE 系统：1天
4. ☑️ 烧录并验证基础功能：半天
5. ☑️ 移植 KMS 应用：2-3天
6. ☑️ 集成 RPMB 安全存储：2-3天
7. ☑️ 测试与调试：3-5天
8. ☑️ 性能优化：1-2天

**总计：** 约 2-3 周

### 风险与注意事项

**潜在风险：**
1. ⚠️ **32位/64位兼容性**
   - 检查所有依赖库是否支持 32位 ARM
   - 注意指针大小变化（8字节 -> 4字节）

2. ⚠️ **性能差异**
   - Cortex-A7 性能弱于模拟的 Cortex-A57
   - 可能需要优化算法（如使用硬件加密加速）

3. ⚠️ **存储容量**
   - RPMB 仅 4MB，需要优化数据结构
   - 考虑分层存储：热数据在 RPMB，冷数据在普通存储

4. ⚠️ **调试难度**
   - 真实硬件调试比 QEMU 困难
   - 建议先在 QEMU ARMv7 环境测试

**缓解措施：**
```bash
# 1. 先在 QEMU ARMv7 环境测试
make PLATFORM=vexpress-qemu_vexpress ARCH=arm

# 2. 使用 GDB 远程调试
arm-none-eabi-gdb -ex "target remote localhost:1234"

# 3. 启用详细日志
export TA_DEV_KIT_DIR=/path/to/ta-dev-kit
export CFG_TEE_CORE_LOG_LEVEL=4
```

---

## 🔗 参考资源

### 官方文档
- **Apache Teaclave TrustZone SDK**: https://teaclave.apache.org/trustzone-sdk-docs/
- **OP-TEE Documentation**: https://optee.readthedocs.io/
- **Trusted Firmware-A**: https://trustedfirmware-a.readthedocs.io/
- **STM32MP1 Wiki**: https://wiki.st.com/stm32mpu/

### GitHub 仓库
- **Teaclave TrustZone SDK**: https://github.com/apache/incubator-teaclave-trustzone-sdk
- **OP-TEE OS**: https://github.com/OP-TEE/optee_os
- **OP-TEE Client**: https://github.com/OP-TEE/optee_client
- **STM32MP1 OP-TEE Build**: https://github.com/STMicroelectronics/optee_os

### 社区与支持
- **Teaclave 邮件列表**: dev@teaclave.apache.org
- **OP-TEE GitHub Discussions**: https://github.com/OP-TEE/optee_os/discussions
- **ST Community**: https://community.st.com/
- **96Boards Forum**: https://discuss.96boards.org/

### 学习资源
- **OP-TEE 101**: https://optee.readthedocs.io/en/latest/architecture/
- **TrustZone Tutorial**: ARM TrustZone Technology Overview
- **RPMB 规范**: JEDEC eMMC 5.1 Standard
- **Secure Boot**: Trusted Firmware-A Design

---

## 📅 更新记录

- **2025-10-16**: 初始版本，基于 OP-TEE 3.x 和 Teaclave SDK 最新文档
- 研究来源：Apache Teaclave 官网、OP-TEE 官方文档、STMicroelectronics 技术文档
