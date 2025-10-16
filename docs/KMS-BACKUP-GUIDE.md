# KMS 钱包自动备份指南

## 📋 概述

KMS 钱包自动备份系统提供了定时备份功能，确保钱包数据的安全性。

## 🔧 安装自动备份

### 快速安装

```bash
# 安装 cron 任务（每小时备份一次）
./scripts/kms-install-backup-cron.sh
```

### 自定义安装

如果需要自定义备份频率，可以手动编辑 crontab：

```bash
# 编辑 cron 任务
crontab -e

# 添加以下行（根据需求修改）
# 每小时执行一次
0 * * * * /path/to/scripts/kms-backup-wallets.sh >> /path/to/logs/kms-backup-cron.log 2>&1

# 每天凌晨2点执行
0 2 * * * /path/to/scripts/kms-backup-wallets.sh >> /path/to/logs/kms-backup-cron.log 2>&1

# 每周日凌晨3点执行
0 3 * * 0 /path/to/scripts/kms-backup-wallets.sh >> /path/to/logs/kms-backup-cron.log 2>&1
```

## 📊 管理自动备份

### 查看当前任务

```bash
# 查看所有 cron 任务
crontab -l

# 仅查看 KMS 备份任务
crontab -l | grep kms-backup
```

### 查看执行日志

```bash
# 查看最新日志
tail -f logs/kms-backup-cron.log

# 查看全部日志
cat logs/kms-backup-cron.log
```

### 手动执行备份

```bash
# 立即执行一次备份
./scripts/kms-backup-wallets.sh
```

### 卸载自动备份

```bash
# 交互式卸载
./scripts/kms-uninstall-backup-cron.sh

# 强制卸载（无确认）
./scripts/kms-uninstall-backup-cron.sh --force
```

## 📁 备份文件管理

### 备份位置

默认备份路径：`~/.kms-backup/`

```bash
# 查看所有备份
ls -lh ~/.kms-backup/

# 查看最新备份
ls -lt ~/.kms-backup/ | head -5
```

### 备份文件格式

```json
{
  "backup_time": "2025-10-16T15:34:22+08:00",
  "version": "2.0",
  "wallets": [
    {
      "wallet_id": "ec3a7490-0fe7-4d5f-951c-ee0457dbdd6b",
      "address": "0x74d4463c39a9e1c1f4e7c77b6c6ed16d59faec65",
      "derivation_path": "m/44'/60'/0'/0/0",
      "mnemonic": "[ENCRYPTED_OR_PLAINTEXT]"
    }
  ]
}
```

### 恢复备份

```bash
# 从备份文件恢复钱包
./scripts/kms-restore-wallets.sh ~/.kms-backup/wallets_backup_20251016_153422.json
```

## ⚠️ 安全建议

1. **加密备份文件**
   ```bash
   # 使用 GPG 加密备份
   gpg -c ~/.kms-backup/wallets_backup_20251016_153422.json
   
   # 解密
   gpg ~/.kms-backup/wallets_backup_20251016_153422.json.gpg
   ```

2. **定期清理旧备份**
   ```bash
   # 仅保留最近7天的备份
   find ~/.kms-backup/ -name "wallets_backup_*.json" -mtime +7 -delete
   ```

3. **异地备份**
   ```bash
   # 复制到远程服务器
   scp ~/.kms-backup/wallets_backup_*.json user@remote:/backup/kms/
   
   # 或使用 rsync
   rsync -avz ~/.kms-backup/ user@remote:/backup/kms/
   ```

4. **权限设置**
   ```bash
   # 限制备份目录权限
   chmod 700 ~/.kms-backup
   chmod 600 ~/.kms-backup/*
   ```

## 🔍 故障排查

### 备份失败

1. 检查 API Server 是否运行
   ```bash
   curl http://localhost:3000/health
   ```

2. 查看错误日志
   ```bash
   tail -50 logs/kms-backup-cron.log
   ```

3. 手动测试备份脚本
   ```bash
   ./scripts/kms-backup-wallets.sh
   ```

### Cron 任务未执行

1. 检查 cron 服务是否运行
   ```bash
   # macOS
   sudo launchctl list | grep cron
   
   # Linux
   systemctl status cron
   ```

2. 检查脚本权限
   ```bash
   ls -l scripts/kms-backup-wallets.sh
   # 应该显示 -rwxr-xr-x
   ```

3. 检查脚本路径
   ```bash
   # 确保 cron 任务中使用绝对路径
   crontab -l | grep kms-backup
   ```

## 📅 Cron 时间格式说明

```
┌───────────── 分钟 (0 - 59)
│ ┌─────────── 小时 (0 - 23)
│ │ ┌───────── 日期 (1 - 31)
│ │ │ ┌─────── 月份 (1 - 12)
│ │ │ │ ┌───── 星期 (0 - 7, 0和7都表示周日)
│ │ │ │ │
* * * * * command
```

### 常用示例

```bash
# 每小时执行
0 * * * * command

# 每天凌晨2点
0 2 * * * command

# 每周一凌晨3点
0 3 * * 1 command

# 每月1号凌晨4点
0 4 1 * * command

# 每6小时
0 */6 * * * command

# 每30分钟
*/30 * * * * command
```

## 📞 支持

如遇问题，请查看：
- 项目 README: `README.md`
- 部署文档: `docs/Deploy.md`
- 变更日志: `docs/Changes.md`
