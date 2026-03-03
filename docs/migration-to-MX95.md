# Migration to i.MX 95 — Secure Storage Comparison

> Created: 2026-03-03 15:07

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
