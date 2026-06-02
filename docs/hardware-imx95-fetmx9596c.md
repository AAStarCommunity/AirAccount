# FET-MX9596-C (NXP i.MX 95) — CA/TA 长期部署评估与方案

> 作者：AirAccount Team | 创建：2026-06-02
> 评估目标：作为 AirAccount KMS（CA + TA）长期生产运行设备
> 参考现有：`docs/migration-to-MX95.md`, `docs/hardware-ARM.md`

---

## 一句话结论

**强烈推荐。** FET-MX9596-C 是目前最适合 AirAccount KMS 长期部署的单板计算机：
- 同架构 (aarch64)，QEMU 已验证代码**零修改**直接烧录运行
- 集成 EdgeLock Secure Enclave（独立安全核心），密钥隔离等级高于前代
- 工业级 15 年寿命，双 10GbE，适合机房 colocation
- RPMB 硬件防回滚存储，解决 REE-FS 的核心安全缺陷

---

## 1. 硬件规格

| 项目 | FET-MX9596-C | 对比：当前 QEMU 环境 |
|------|-------------|-------------------|
| **SoC** | NXP i.MX 95 (MCIMX95) | 模拟 cortex-a57 |
| **CPU** | 6× Cortex-A55 @ 2.0GHz (ARMv8.2-A) | 2× emulated A57 |
| **实时核** | 1× Cortex-M7 @ 800MHz + 1× Cortex-M33 @ 333MHz | 无 |
| **NPU** | 2 TOPS eIQ Neutron（可用于异常检测） | 无 |
| **内存** | 最高 8 GB LPDDR4x | 1057 MB (QEMU) |
| **存储** | 64 GB eMMC（含 RPMB）+ microSD | QEMU 磁盘镜像 |
| **网络** | 双 10GbE | QEMU virtio-net |
| **接口** | PCIe Gen3, CAN, RS485, USB 3.0 | 9p + virtio |
| **温度范围** | −40°C ～ +85°C（工业级） | N/A |
| **寿命保证** | NXP 15年（2040年前） | N/A |
| **安全子系统** | EdgeLock Secure Enclave（独立 M33） | 软件模拟 |

**关键点：** aarch64 与 QEMU 完全相同，编译目标 `aarch64-unknown-optee` / `aarch64-unknown-linux-gnu` 无需变更。

---

## 2. 安全能力评估

### 2.1 TrustZone + OP-TEE

i.MX 95 完整支持 ARMv8 TrustZone：

```
Normal World (Linux + CA)
  ↕ SMC / TEE Supplicant
Secure World (OP-TEE + TA)
  ↕ TZASC (TrustZone Address Space Controller)
EdgeLock Secure Enclave (独立 M33，不可访问)
```

- **TZASC**: 硬件内存隔离，Normal World 完全无法读取 Secure World 内存
- **TRDC/XRDC**: Resource Domain Controllers，细粒度外设安全分配
- **HUK**: Hardware Unique Key，由 EdgeLock Enclave 管理，OP-TEE 用于派生存储加密密钥

### 2.2 EdgeLock Secure Enclave（vs 前代 TrustZone-only）

| 能力 | STM32MP157 (TrustZone-only) | i.MX 95 (EdgeLock Enclave) |
|------|---------------------------|--------------------------|
| 密钥管理 | OP-TEE 软件 HUK | EdgeLock 硬件保管 HUK |
| 安全启动 | TF-A HAB | **AHAB**（更强签名链） |
| 证明/Attestation | 无原生支持 | EdgeLock 原生支持 |
| 远程密钥配置 | 无 | EdgeLock 2 GO 服务 |
| 独立性 | 与 A7 核共享存储控制 | **独立 M33，不可从 Normal World 直接访问** |
| 侧信道防护 | 有限 | 增强（ARMv8.2 spectre 缓解） |

对 AirAccount KMS 意义：
1. TA 私钥最终被 EdgeLock 保护的 HUK 加密，即使 OP-TEE 被攻破也无法离开 SoC
2. AHAB 保证启动链完整性：ROM → SPL → U-Boot → OP-TEE → Linux → TA
3. 可选接 SE050（EAL 6+）作第二层隔离（当前不需要，未来可扩展）

### 2.3 RPMB 安全存储（核心升级点）

**当前 QEMU 问题**：SQLite DB 存于 QEMU 虚拟磁盘，TA 的 OP-TEE Secure Storage 使用 REE-FS（普通文件系统加密）。

**i.MX 95 升级**：eMMC 含 RPMB（Replay Protected Memory Block）分区：

```
OP-TEE Secure Storage API
  ↓
tee-supplicant (RPMB backend)
  ↓
/dev/mmcblk0rpmb
  ↓ 写入时附带 HMAC-SHA256 + 单调递增写计数器
eMMC RPMB 分区（焊死在板上，不可拆除）
```

| 攻击向量 | REE-FS（QEMU/DK2） | RPMB（i.MX 95） |
|---------|------------------|----------------|
| 数据读取 | HUK 加密保护 | HUK 加密 + 硬件隔离 |
| 数据篡改 | HMAC 检测 | 硬件 MAC + 写计数器 |
| **回滚攻击** | ❌ 脆弱（可恢复旧文件） | ✅ 防护（写计数器单调递增） |
| **物理克隆** | ❌ SD/虚拟盘可复制 | ✅ 防护（eMMC 焊死）|
| 离线分析 | ❌ 文件可导出分析 | ✅ 困难 |

**对 AirAccount 重要性极高**：WebAuthn passkey（私钥）存于 TA Secure Storage，RPMB 确保即使物理盗取设备也无法克隆钱包。

### 2.4 安全启动链（AHAB）

```
i.MX 95 Boot ROM
  ↓ 验证 SRK 融丝签名
SPL (Secondary Program Loader)
  ↓ 验证 U-Boot FIT image
U-Boot v2025.04
  ↓ 验证 OP-TEE + Linux kernel
OP-TEE (Secure World Monitor)
  ↓ 加载并验证 TA 签名
AirAccount TA (4319f351-...)
  ↓
Linux + CA (kms-api-server)
```

每级都有数字签名验证，SRK 私钥由 AirAccount 团队保管，公钥 hash 熔丝烧录到 SoC。

---

## 3. 与现有 QEMU 环境的兼容性

这是选择 i.MX 95 的最重要理由：**零代码修改**。

| 组件 | QEMU 目标 | i.MX 95 目标 | 需要变更？ |
|------|----------|-------------|---------|
| TA 编译 | `aarch64-unknown-optee` | `aarch64-unknown-optee` | ❌ 否 |
| CA 编译 | `aarch64-unknown-linux-gnu` | `aarch64-unknown-linux-gnu` | ❌ 否 |
| OP-TEE API | TEE Client API 2.0 | TEE Client API 2.0 | ❌ 否 |
| TA 签名 | `sign_encrypt.py` + RSA-4096 | 同上 + AHAB outer wrapper | 生产需加 AHAB |
| 启动方式 | `bl1.bin` QEMU bios | U-Boot + TF-A + OP-TEE FIT | BSP 构建 |
| Secure Storage | REE-FS (文件) | **RPMB** (硬件) | tee-supplicant 配置 |
| 数据库 | `/data/kms/kms.db` | `/data/kms/kms.db` | ❌ 否 |
| 网络暴露 | QEMU hostfwd → Cloudflare | 直连 → Cloudflare | 简化！ |

**对比 STM32MP157F-DK2**（旧推荐方案）：DK2 是 ARMv7-A（32-bit），需要改变编译 target triple、重新处理 p256-m 32bit 兼容性；i.MX 95 完全跳过这些痛点。

---

## 4. 部署方案

### 4.1 总体架构

```
Internet
  ↓ HTTPS
Cloudflare Tunnel (cloudflared 进程)
  ↓ localhost:3000
kms-api-server (Normal World, Linux, CA)
  ↓ TEEC_InvokeCommand via /dev/tee0
tee-supplicant
  ↓ TEE Client API
OP-TEE Secure World
  ↓ TA_InvokeCommandEntryPoint
AirAccount TA (4319f351-..)
  ↓ RPMB Secure Storage
EdgeLock Enclave (HUK) → eMMC RPMB
```

### 4.2 BSP 构建环境（meta-imx）

使用 NXP 官方 Yocto BSP：

```bash
# meta-imx LF6.18.2_1.0.0 (2026-03-xx release)
# 在 x86_64 Linux 构建机上执行

# 1. 安装 repo 工具
mkdir ~/imx95-bsp && cd ~/imx95-bsp
repo init -u https://github.com/nxp-imx/imx-manifest.git \
  -b imx-linux-scarthgap -m imx-6.18.2-1.0.0.xml
repo sync -j8

# 2. 初始化 Yocto 构建环境
DISTRO=fsl-imx-xwayland MACHINE=imx95lp4x15-19x19-evk \
  source imx-setup-release.sh -b build-imx95

# 3. 添加 meta-teaclave / meta-optee（自定义 TA layer）
# ... 见下方 4.3

# 4. 构建完整系统镜像
bitbake imx-image-full
```

**构建机要求**：Ubuntu 22.04 x86_64，64GB RAM，500GB SSD，4 小时首次构建。

### 4.3 OP-TEE + TA 集成到 Yocto

```
# meta-aastar-kms/ (自定义 Yocto layer)
meta-aastar-kms/
├── conf/layer.conf
├── recipes-security/
│   ├── optee-os/
│   │   └── optee-os_%.bbappend        # 启用 CFG_RPMB_FS=y
│   ├── optee-client/
│   │   └── optee-client_%.bbappend   # RPMB backend
│   └── aastar-ta/
│       └── aastar-ta.bb              # 打包 TA + 签名
└── recipes-apps/
    └── kms-api-server/
        └── kms-api-server.bb         # CA 应用 + systemd service
```

关键 OP-TEE 编译选项：
```bash
# optee-os_%.bbappend
EXTRA_OEMAKE += " \
  CFG_RPMB_FS=y \
  CFG_RPMB_TESTKEY=n \
  CFG_RPMB_WRITE_KEY=y \
  CFG_CORE_ASLR=y \
  CFG_CORE_RWDATA_NOEXEC=y \
  CFG_STACK_GUARD_PAGE=y \
"
```

### 4.4 TA 打包与签名

开发阶段（用 OP-TEE 默认 key）：
```bash
# 沿用 QEMU 现有流程
python3 $TA_DEV_KIT_DIR/scripts/sign_encrypt.py sign-enc \
  --uuid 4319f351-0b24-4097-b659-80ee4f824cdd \
  --ta-version 1 \
  --in stripped_ta \
  --out 4319f351-0b24-4097-b659-80ee4f824cdd.ta \
  --key $TA_DEV_KIT_DIR/keys/default_ta.pem
```

生产阶段（AHAB + 自有 signing key）：
```bash
# 额外步骤：用 NXP CST (Code Signing Tool) 对 FIT image 签名
# 使用 SRK key（离线 HSM 保管）
cst --o ahab_signed_fit.bin --cmd sign_ahab ...
```

### 4.5 初始化 RPMB（一次性操作）

首次部署时，RPMB 写密钥必须在 OP-TEE 侧初始化：
```bash
# 在 imx95 板上执行（仅一次！之后无法更改）
tee-supplicant &
# OP-TEE 自动生成并写入 RPMB 认证 key（源自 HUK）
# 验证
ls /sys/kernel/debug/optee/supp_plugin/  # 应出现 rpmb_fs
```

### 4.6 CA 部署（systemd）

```ini
# /etc/systemd/system/kms-api-server.service
[Unit]
Description=AirAccount KMS API Server v0.19.0
After=network-online.target tee-supplicant.service optee.service
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/bin/kms-api-server
Restart=always
RestartSec=10
User=root
Environment=KMS_DB_PATH=/data/kms/kms.db
Environment=KMS_ORIGIN=https://kms.aastar.io
WorkingDirectory=/data/kms
StandardOutput=journal
StandardError=journal

# 硬化
CapabilityBoundingSet=CAP_NET_BIND_SERVICE
PrivateTmp=yes
ProtectHome=yes
NoNewPrivileges=yes

[Install]
WantedBy=multi-user.target
```

### 4.7 Cloudflare Tunnel（保持现有方案）

```bash
# cloudflared 作为 systemd service（与 QEMU 方案相同）
# 区别：不再需要 Docker port-forward + QEMU hostfwd 两层转发
# 直接：Cloudflare Tunnel → localhost:3000 → kms-api-server
cloudflared service install <token>
```

网络路径对比：
```
# 旧（QEMU）
Browser → Cloudflare → Mac:3000 → Docker:3000 → QEMU guest:3000 → kms-api-server
         延迟约 +20-50ms，3层转发

# 新（i.MX 95）  
Browser → Cloudflare → imx95:3000 → kms-api-server
         延迟降低，2跳直连
```

### 4.8 监控与运维

```bash
# M7 实时核运行轻量级 watchdog（Zephyr RTOS 或 FreeRTOS）
# 监控 CA 进程心跳，异常时触发 Linux 重启
# 利用 imx95 独特的 M7/M33 异构特性，不影响 CA 性能

# 日志
journalctl -u kms-api-server -f

# TA secure world 日志（通过 /dev/tee0 debug interface）
cat /sys/kernel/debug/optee/call_count

# 性能基准（预期）
# - CreateKey:   50-80ms  (vs QEMU ~80-120ms, vs DK2 ~200ms)
# - Sign:        10-20ms  (vs QEMU ~20-40ms, vs DK2 ~50-100ms)
# - RPMB write:  +5-15ms overhead vs REE-FS
```

---

## 5. 迁移路径（从 QEMU 到 i.MX 95）

### Phase 0：购买与基础验证（1-2 周）

```
1. 购买 FET-MX9596-C SOM + 开发载板
   - 供应商：Forlinx (forlinx.net) 或淘宝/京东代理
   - 预算：¥2000-4000（含载板）
   - 注意：确认含 eMMC（RPMB 需要）

2. 刷入 NXP 官方 BSP（预编译 EVK 镜像先跑通）
   - 下载：NXP i.MX 95 EVK SD Card Image
   - 验证 OP-TEE 正常启动：dmesg | grep -i optee
   - 验证 RPMB：ls /dev/mmcblk*rpmb
```

### Phase 1：QEMU → i.MX 95 代码直接运行（3-5 天）

```
1. 在 x86_64 构建机上交叉编译（保持现有 Docker 环境）
   # TA（无变化）
   cd kms/ta
   CC=aarch64-linux-gnu-gcc xargo build \
     --target aarch64-unknown-optee --release

   # CA（无变化）  
   cd kms/host
   cargo build --target aarch64-unknown-linux-gnu --release \
     --bin kms-api-server

2. 通过 scp 部署到 imx95（替代 QEMU 9p 共享目录）
   scp 4319f351-*.ta root@imx95:/lib/optee_armtz/
   scp kms-api-server root@imx95:/usr/bin/
   # 重启 tee-supplicant 使新 TA 生效
   systemctl restart tee-supplicant
   
3. 启动并验证
   KMS_DB_PATH=/data/kms/kms.db ./kms-api-server
   curl http://imx95-ip:3000/health
```

### Phase 2：RPMB 存储集成（1 周）

```
1. 验证 tee-supplicant RPMB backend 正常工作
2. TA 侧 Secure Storage 已通过 OP-TEE API 自动使用 RPMB（无需改代码）
   # 创建 key 后验证存储位置
   ls /sys/kernel/debug/optee/  # 无明文文件（不同于 REE-FS）
3. 压力测试 RPMB：创建 1000 个 wallet，验证回滚保护
```

### Phase 3：生产 AHAB 签名（2-3 天）

```
1. 生成 AirAccount 专属 SRK（Super Root Key）
   # 离线 air-gapped 机器上
   openssl genrsa -out srk.pem 4096
   # 提取公钥 hash，熔断到 SoC fuse（不可逆！）
   
2. 用 NXP CST 签名 U-Boot / OP-TEE FIT image
3. 测试签名后能否正常启动
4. 最终封板（fuse CLOSE 位），拒绝未签名镜像
```

### Phase 4：机房部署（按需）

```
1. 将 imx95 设备送入 colocation 机房（东南亚优选新加坡/泰国）
2. 配置 cloudflared tunnel
3. 监控接入（Prometheus + Grafana，现有方案不变）
4. DNS 切换：kms.aastar.io → tunnel
```

---

## 6. 充分利用 i.MX 95 特性

### 6.1 M7 实时核：TEE 独立 Watchdog

```
[Normal World: Linux + CA]
[Secure World: OP-TEE + TA]
[M7 实时核: FreeRTOS watchdog]
  - 每 5 秒收 CA 心跳包
  - 超时未收到 → 触发 Linux reset（不影响 Secure World 密钥）
  - CA 崩溃时自动重启，密钥安全保留
```

实现：通过 RPMsg（M7↔A55 IPC 框架）通信，无需外部硬件 watchdog。

### 6.2 NPU：行为异常检测（可选）

2 TOPS NPU 可运行轻量级异常检测模型：
- 监控 API 请求频率模式
- 检测高频暴力枚举（比纯软件 rate limiter 更低延迟）
- 未来可集成 UEBA（User Entity Behavior Analytics）

### 6.3 双 10GbE：高可用网络

```
eth0 → Cloudflare Tunnel（主）
eth1 → 管理网络（带外管理，SSH + 监控）
```

主备分离，管理流量不占用服务带宽。

### 6.4 EdgeLock 2 GO：远程密钥配置

NXP 提供的远程安全配置服务，可以：
- 远程注入 AirAccount 的 SRK 证书（不需要物理接触设备）
- 适合未来大规模部署多台 imx95 节点

---

## 7. 风险与对策

| 风险 | 等级 | 对策 |
|------|------|------|
| BSP 构建复杂度 | 中 | 先用 NXP 预编译镜像验证，再做定制 |
| RPMB 写密钥丢失 | 高 | 首次初始化前备份 efuse，离线保管 SRK |
| TA 签名变更 | 低 | 测试网用 default_ta.pem，生产用 SRK 签名 |
| QEMU vs 真实 HW 行为差异 | 低 | RPMB 是主要差异；Phase 1 快速验证 |
| 设备 EOL | 极低 | NXP 15年供货保证至 2040 年 |

---

## 8. 总结与推荐

| 维度 | 评分 | 说明 |
|------|------|------|
| 安全等级 | ★★★★★ | EdgeLock + RPMB + AHAB，当前最强消费级安全 |
| 兼容性 | ★★★★★ | aarch64，代码零修改 |
| 迁移成本 | ★★★★☆ | BSP 构建需要 1 周学习，运行代码 1-2 天 |
| 长期可用性 | ★★★★★ | 15年，工业级，NXP 直接支持 |
| 性能 | ★★★★☆ | 6× A55 + 2 TOPS，远超需求 |
| 价格 | ★★★☆☆ | SOM ¥1500-2500，载板额外 ¥500-1000 |

**推荐行动**：购买 FET-MX9596-C + 开发载板，按 Phase 0-2 路径在 2-3 周内完成验证。QEMU 环境继续作为开发/CI 环境，imx95 作为生产环境。

---

## 参考资源

- [Forlinx FET-MX9596-C 官方页面](https://www.forlinx.net/product/imx95-c-system-on-module-151.html)
- [NXP i.MX 95 Fact Sheet](https://www.nxp.com/docs/en/fact-sheet/IMX95FS.pdf)
- [OP-TEE on i.MX 95 (The Good Penguin)](https://www.thegoodpenguin.co.uk/blog/secure-storage-with-i-mx-95-verdin-evk-using-trusted-keys-with-op-tee/)
- [NXP meta-imx BSP](https://github.com/nxp-imx/meta-imx)
- [OP-TEE NXP 平台文档](https://optee.readthedocs.io/en/latest/architecture/platforms/nxp.html)
- [NXP AHAB AN12312 (PKI 树生成)](https://www.nxp.com/docs/en/application-note/AN12312.pdf)
- [EdgeLock 2 GO](https://www.nxp.com/products/security-and-authentication/authentication/edgelock-2go:EDGELOCK-2GO)
- 项目内：`docs/migration-to-MX95.md`（存储对比）, `docs/hardware-ARM.md`（旧推荐方案，已被本文取代）
