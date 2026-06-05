# Migration to i.MX 93 / i.MX 95 — Secure Storage Comparison

> Created: 2026-03-03 15:07
> Updated: 2026-06-04

## 默认硬件决策（2026-06-04 确认）

**结论：下一代硬件默认选 i.MX93，不等 i.MX95。**

理由：
- i.MX93 在国内供货正常，i.MX95 到货周期过长。
- 对于 AirAccount KMS 当前场景（TEE 内生成私钥、TEE 内签名、私钥不出 TEE），i.MX93 是相对 DK2 的质变升级（32→64位、REE-FS→RPMB/eMMC、独立 EdgeLock Secure Enclave）。
- i.MX93 → i.MX95 是量变，安全模型相同，主要差异是 Advanced Profile 和更高吞吐；对单节点 KMS 意义不大。
- 后续如有生产级硬件安全认证需求，代码改动极小（重新编译 + 换 BSP），可随时升 MX95。

迁移代码改动要点（DK2 → i.MX93）：
- CA target: `armv7-unknown-linux-gnueabihf` → `aarch64-unknown-linux-gnu`
- TA target: `arm-unknown-optee` → `aarch64-unknown-optee`
- 验证 p256-m aarch64 编译 flags
- NXP BSP + OP-TEE（替换 ST 那套）
- `tee-supplicant` 启用 RPMB backend
- systemd service 补 `WorkingDirectory=/data/kms`

---

## 2026-06-04 Decision Note: DK2 vs i.MX93 vs i.MX95

相对当前 STM32MP157F-DK2，i.MX93 / i.MX95 在 AirAccount 的 TEE 私钥存储场景里是明显升级；但如果只做 KMS、TEE 内生成私钥、TEE 内签名、私钥不出 TEE，i.MX93 和 i.MX95 之间的差距没有 DK2 到 i.MX93 那么大，i.MX93 很可能够用。

当前仓库测试记录基于 `KMS-stm32` 分支，设备为 STM32MP157F-DK2，记录 CPU 为 Cortex-A7 650MHz，见 `full-test-result-3-3-2026.md`。ST 官方 DK2 是双 Cortex-A7 32-bit + Cortex-M4，板载 microSD，参考 ST DK2: <https://www.st.com/en/evaluation-tools/stm32mp157f-dk2.html>。本文件下方也记录了 DK2 当前是 SD 卡，没有 RPMB，OP-TEE secure storage 回退到 REE-FS。

| 项目 | STM32MP157F-DK2 当前 | i.MX93 | i.MX95 |
|------|----------------------|--------|--------|
| TEE 基础 | 有 OP-TEE / TrustZone | 有 OP-TEE / TrustZone | 有 OP-TEE / TrustZone |
| CPU 架构 | ARMv7-A 32-bit Cortex-A7 | ARMv8-A 64-bit Cortex-A55 | ARMv8-A 64-bit Cortex-A55 |
| 安全根 | 主要靠 TrustZone + SoC HUK | EdgeLock Secure Enclave | EdgeLock Secure Enclave Advanced Profile |
| Secure Storage | SD 卡上的 REE-FS，加密但弱防回滚 | 通常可走 eMMC/RPMB，取决于板卡/BSP | eMMC/RPMB，更适合生产 |
| 防克隆/防回滚 | 弱，SD 卡可复制/回滚 | 明显更好 | 更强 |
| 性能 | 当前 SignHash 约 1.2s，派生可到几十秒 | 会明显提升 | 提升更大，但对单纯 KMS 未必必要 |
| 迁移工作 | 当前已跑通 | 需要 64-bit target + NXP BSP/OP-TEE | 同 i.MX93，但 BSP/安全栈更复杂 |

仓库当前实现方式是标准 OP-TEE TA/CA：CA 通过 `optee_teec` 打开 TA session 调命令，见 `src/eth_wallet.rs`；TA 内生成钱包熵和随机 ID，见 `kms/ta/src/wallet.rs`，再通过 `SecureStorageClient` 持久化。OP-TEE secure storage 底层可以是 REE-FS 或 RPMB；`docs/optee-storage-analysis.md` 已记录真实硬件上 RPMB 有单调计数器、防回滚能力。

建议：

- 如果目标只是 TEE 内生成私钥、TEE 内签名、私钥不出 TEE，i.MX93 已经是比 DK2 大幅升级，通常够用。NXP 资料也提到在 i.MX93/i.MX95 生态中使用 OP-TEE/PKCS#11 做安全 key/certificate 存储，并以 EdgeLock Secure Enclave 作为硬件 root of trust，参考 NXP training: <https://www.nxp.com/design/design-center/training/TIP-HOW-CREATE-SECURE-SYSTEMS-IMX95>。
- 如果目标是生产级硬件安全、强防回滚、防克隆、更高吞吐，或后续可能做车规/工业安全认证，i.MX95 更合适。NXP i.MX95 官方资料说明 EdgeLock Secure Enclave Advanced Profile 提供硬件 root-of-trust、secure boot、secure debug/update、实时签名、认证和加密能力，参考 NXP i.MX95: <https://www.nxp.com/products/i.MX95>。
- 硬件升级不能弥补 API 把密钥材料吐出 TEE 的问题。生产发布前必须把 mnemonic/private key export 做成 dev/test-only feature，或彻底禁掉；开发调试阶段可以显式启用，生产构建和发布流水线必须禁止。具体方案见 `docs/secret-export-feature-plan.md`。

## FRDM-IMX93 Board Validation Plan

收到 FRDM-IMX93 后需要验证三层，不要只看 Cortex-A55 或 i.MX93 名称：

1. 硬件层：确认板卡确实是 FRDM-IMX93，SoC 是 i.MX93，板载存储是 eMMC 5.1，并存在 RPMB 分区。
2. BSP/OP-TEE 层：确认镜像中 OP-TEE 已启用，并且 OP-TEE OS 构建配置启用了 RPMB secure storage backend，例如 `CFG_RPMB_FS=y`。如果没有启用，OP-TEE secure storage 可能仍会回退到 REE-FS。
3. 运行时层：确认 `tee-supplicant` 正常运行，TA 能创建 secure storage object，并确认这些对象不是只落在 `/data/tee` 的 REE-FS fallback。

板子启动后先收集这些信息：

```bash
cat /proc/device-tree/model
uname -a
ls -l /dev/tee* /dev/mmcblk* /dev/mmcblk*rpmb 2>/dev/null
dmesg | grep -Ei "optee|tee|rpmb|mmc|trustzone"
ps | grep -E "tee-supplicant|tee_supplicant" | grep -v grep
mount | grep -Ei "tee|rpmb|data"
ls -la /data/tee 2>/dev/null
```

如果看到 `/dev/mmcblk0rpmb` 或类似设备，只能说明 eMMC RPMB 设备存在；还不能证明 OP-TEE secure storage 正在使用 RPMB。下一步要进入 BSP/OP-TEE 构建目录或查看镜像构建日志，确认配置：

```bash
grep -R "CFG_RPMB_FS" -n build tmp deploy 2>/dev/null
grep -R "CFG_REE_FS" -n build tmp deploy 2>/dev/null
grep -R "RPMB" -n build tmp deploy 2>/dev/null
```

预期目标：

- `CFG_RPMB_FS=y`
- 不把生产 secure storage 仅依赖在 `CFG_REE_FS=y`
- `tee-supplicant` 可访问 RPMB 设备
- KMS TA 创建钱包、重启后钱包仍可读取
- 尝试回滚 `/data/tee` 文件不能回滚钱包状态；如果能通过复制旧 `/data/tee` 回滚，说明仍在 REE-FS 或配置不完整

验证 AirAccount KMS 时按这个顺序：

1. 部署生产构建 TA，确认没有启用 `export-secrets`。
2. 调用 CreateWallet/CreateKey，确认返回值不包含 mnemonic 明文。
3. 调用 DeriveAddress/SignHash，确认私钥可在 TEE 内正常派生和签名。
4. 直接调用 `ExportPrivateKey` command id，确认生产 TA 返回 disabled error。
5. 重启系统后再次调用 DeriveAddress/SignHash，确认 secure storage 持久化。
6. 如需验证防回滚，先记录钱包计数器或派生地址状态，再尝试恢复旧 `/data/tee` 内容；生产目标下状态不应被旧 REE-FS 文件回滚。

## Storage Architecture: DK2 vs i.MX 95

| 特性 | STM32MP157-DK2 (SD) | i.MX 95 (eMMC/UFS) |
|------|---------------------|---------------------|
| 存储介质 | SD Card | eMMC / UFS |
| RPMB 支持 | **无** | **原生支持** |
| 安全核心 | Cortex-A7 (TrustZone) | EdgeLock Secure Enclave (独立) |
| 存储方式 | REE-FS (加密文件) | Hardware-bound RPMB |
| 防克隆/回滚 | 较弱 | **极强** |

## DK2 当前存储方式

DK2 使用 **SD 卡**（`mmc0: SDHC`），没有 eMMC，因此**没有 RPMB 设备**（`/dev/mmcblk0rpmb` 不存在）。

OP-TEE 的 secure storage 回退到 **REE-FS 模式**：
- 文件存储在 `/data/tee/` 目录（Normal World 文件系统）
- `tee-supplicant` 代理所有 I/O 操作
- 数据用 **HUK (Hardware Unique Key)** 派生的密钥加密
- `dirf.db` 是目录元数据，编号文件（`0`, `1`, `2a`, ...）是实际对象

### 安全性限制

| 威胁 | REE-FS (DK2) | RPMB (i.MX 95) |
|------|-------------|-----------------|
| 数据读取 | HUK 加密保护 | HUK 加密 + 硬件隔离 |
| 数据篡改 | HMAC 检测 | 硬件 MAC + 写计数器 |
| 回滚攻击 | **脆弱** — 攻击者可恢复旧文件 | **防护** — RPMB 写计数器单调递增 |
| 物理克隆 | **脆弱** — SD 卡可整盘复制 | **防护** — RPMB 绑定到 eMMC 芯片 |
| 离线分析 | **可能** — SD 卡可拔出分析 | **困难** — eMMC 焊死在板上 |

### CA 端 DB 路径问题

当前 `kms-api-server` 的 systemd service 没有设置 `WorkingDirectory`，默认 CWD 是 `/`，导致 SQLite DB 创建在 `/kms.db`。

**建议迁移时修复**：
```ini
[Service]
WorkingDirectory=/data/kms
Environment=KMS_DB_PATH=/data/kms/kms.db
```

## i.MX 95 迁移要点

### 硬件差异

| 项目 | STM32MP157F-DK2 | i.MX 95 |
|------|-----------------|---------|
| CPU | 2× Cortex-A7 @ 650MHz | 6× Cortex-A55 @ 2.0GHz |
| 架构 | ARMv7-A (32-bit) | ARMv8.2-A (64-bit) |
| 安全核心 | ARM TrustZone (shared A7) | EdgeLock Secure Enclave (独立 M33) |
| Secure Storage | REE-FS on SD card | **RPMB on eMMC** |
| 预期性能 | SignHash ~1.26s (含 p256-m verify) | **~100-150ms** (8-10x) |

### 迁移工作

1. **Target triple**: `arm-unknown-optee` → `aarch64-unknown-optee`
2. **TA 构建**: xargo → cargo (64-bit OP-TEE 可能直接支持 std)
3. **p256-m**: 64-bit 适配（当前 32-bit limbs，需验证 aarch64 下编译 flags）
4. **OP-TEE 版本**: 可能从 3.x 升级到 4.x（API 差异）
5. **RPMB 配置**: `tee-supplicant` 启用 RPMB backend
6. **CA 编译**: `armv7-unknown-linux-gnueabihf` → `aarch64-unknown-linux-gnu`
