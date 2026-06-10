# KMS 备份与恢复指南

## 一句话机制

**首次运行做全量备份，之后每次做增量备份**（rsync `--link-dest`：未变更文件用硬链接，只传输新增/修改的内容）。每个备份目录看起来都是完整的，但磁盘占用只增加变化量。

---

## 备份了什么 / 没备份什么

| 内容 | 路径 | 备注 |
|------|------|------|
| ✅ 钱包元数据 DB | `/root/AirAccount/kms.db` | SQLite，含钱包地址和 ID，**不含私钥** |
| ✅ TA 二进制 | `/lib/optee_armtz/<uuid>.ta` | TEE 可信应用，重新编译需要完整 OP-TEE SDK |
| ✅ CA 二进制 | `/root/AirAccount/target/release/kms-api-server` | Host 端 HTTP 服务，免去重新交叉编译 |
| ✅ systemd 服务配置 | `/etc/systemd/system/kms-api.service` | 含 dirf-repair 依赖和所有环境变量 |
| ✅ Cloudflare Tunnel 配置 | `/etc/cloudflared/` 或 `/root/.cloudflared/` | 公网 HTTPS 端点的证书和配置 |
| ✅ 部署/运维脚本 | `/root/AirAccount/kms/scripts/` | backup.sh 本身也在其中 |
| ✅ 系统信息快照 | 每个备份目录内 `system-info.txt` | 内核版本、OP-TEE 版本、服务状态、DB 行数 |
| ❌ TEE 安全存储 | `/var/lib/tee/` | **硬件绑定加密，复制到任何其他设备均为无用密文，备份无意义** |
| ❌ RPMB 分区 | `/dev/mmcblk0rpmb` | 硬件绑定，无法有意义地块复制 |
| ❌ 任何密钥文件 | `*.key *.pem *.priv` | rsync `--exclude` 强制过滤 |

> **重要**：私钥永远不离开 TEE。备份的意义是"快速恢复运行环境"，而不是"恢复私钥"。如果硬件（MX93 主板）损坏，私钥无法从备份中还原——这是 TEE 的设计目标。

---

## 备份存储结构

```
/root/backups/kms/
  2026-06-10_030000/        ← 时间戳目录
    files/                  ← 实际备份文件（rsync 硬链接增量）
      root/AirAccount/kms.db
      lib/optee_armtz/<uuid>.ta
      root/AirAccount/target/release/kms-api-server
      etc/systemd/system/kms-api.service
      etc/cloudflared/      （如有）
      root/AirAccount/kms/scripts/
      system-info.txt
    manifest.sha256         ← 所有文件的 SHA-256 校验和
    backup-info.txt         ← 备份类型、耗时、文件数、大小
  latest -> 2026-06-10_030000  ← 指向最新备份的符号链接
```

**轮转策略**（自动）：保留最近 30 天 + 每月最新 1 个（保留 12 个月）+ 每年最新 1 个（保留 5 年）。

---

## 如何执行备份

### 方式一：安装 systemd 定时器（推荐，部署到 MX93 后执行一次）

```bash
# 安装：启动后 5 分钟首次运行，之后每小时自动备份
sudo ./kms/scripts/install-backup-timer.sh

# 可选：同时推送到远程 SSH 服务器
sudo ./kms/scripts/install-backup-timer.sh --remote user@backup-server:/backups/mx93

# 检查定时器状态
systemctl status kms-backup.timer

# 查看备份日志
journalctl -u kms-backup -f
# 或
tail -f /var/log/kms-backup.log
```

### 方式二：手动执行

```bash
# 普通增量备份（第一次自动变全量）
sudo ./kms/scripts/backup.sh

# 强制全量备份
sudo ./kms/scripts/backup.sh --full

# 先预演，不真正写入
sudo ./kms/scripts/backup.sh --dry-run

# 备份后推送到远程
sudo ./kms/scripts/backup.sh --remote user@192.168.1.100:/backups/mx93
```

---

## 如何恢复

```bash
# 1. 列出所有可用备份
sudo ./kms/scripts/backup-restore.sh --list

# 2. 先 dry-run 确认恢复内容
sudo ./kms/scripts/backup-restore.sh --backup latest --dry-run

# 3. 恢复到原始路径（覆盖现有文件）
sudo ./kms/scripts/backup-restore.sh --backup latest

# 4. 恢复到临时目录（用于检查，不影响运行环境）
sudo ./kms/scripts/backup-restore.sh --backup 2026-06-10_030000 --dest /tmp/restore-test

# 恢复后重启服务
sudo systemctl daemon-reload
sudo systemctl restart kms-api.service
```

恢复脚本会自动：
- 用 SHA-256 manifest 校验备份完整性（可用 `--no-verify` 跳过）
- 跳过 `/var/lib/tee/`（TEE 存储不可恢复，即使存在也不会覆盖）
- 过滤 `*.key *.pem *.priv`

---

## 卸载定时器

```bash
sudo ./kms/scripts/install-backup-timer.sh --uninstall
```

---

## 常见问题

**Q: 主板损坏，私钥还能找回吗？**
不能。私钥存在 TEE 安全存储里，硬件绑定，无法从备份恢复。这是 TEE 的安全保证，不是缺陷。

**Q: 首次备份有多大？**
主要取决于 CA 二进制（约 20–50 MB）。后续增量备份通常只有 kms.db 的变化量（KB 级）。

**Q: 备份能用于新主板吗？**
二进制文件（TA/CA）和配置文件可以恢复到新板，帮助快速搭建运行环境。但 `/var/lib/tee/` 里的钱包密钥是不可迁移的——新板需要重新创建钱包。

**Q: 如何验证备份是否完好？**
```bash
# 手动校验最新备份的 SHA-256 manifest
cd /root/backups/kms/latest
sha256sum -c manifest.sha256
```
