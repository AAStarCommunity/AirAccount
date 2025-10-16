# KMS 钱包持久化解决方案

## 问题

QEMU 重启后，OP-TEE Secure Storage 中的所有数据（私钥、助记词）都会丢失，导致：
- 开发测试时需要重新创建钱包
- 地址每次都不同，不方便测试
- 无法保留重要的测试数据

## 解决方案

提供三种工具来解决持久化问题：

### 1. 开发测试钱包（推荐用于日常开发）

**脚本**: `scripts/kms-init-dev-wallets.sh`

**原理**: 使用固定的助记词创建测试钱包，每次重启后用相同助记词重新创建，地址保持不变。

**使用方式**:
```bash
# 启动 KMS
./scripts/kms-auto-start.sh

# 等待 API Server 启动（15 秒）
sleep 15

# 初始化开发测试钱包
./scripts/kms-init-dev-wallets.sh
```

**固定的测试钱包**:
1. `dev-wallet-1`: 助记词 `test test test test test test test test test test test junk`
2. `dev-wallet-2`: 助记词 `abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about`
3. `dev-wallet-3`: 助记词 `zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong`

**优点**:
- ✅ 地址固定，方便测试
- ✅ 重启后自动恢复（重新创建）
- ✅ 安全（仅用于测试，不含真实资金）

**缺点**:
- ⚠️ 仅适用于开发测试
- ⚠️ 不能用于生产环境

### 2. 钱包备份与恢复

**备份脚本**: `scripts/kms-backup-wallets.sh`
**恢复脚本**: `scripts/kms-restore-wallets.sh`

**原理**: 通过 API 导出所有钱包的私钥和助记词到 JSON 文件，重启后重新导入。

**使用方式**:

```bash
# 1. 备份当前所有钱包
./scripts/kms-backup-wallets.sh

# 备份文件保存在: ~/.kms-backup/wallets_backup_YYYYMMDD_HHMMSS.json

# 2. 重启 KMS 后恢复
./scripts/kms-auto-start.sh
sleep 15
./scripts/kms-restore-wallets.sh ~/.kms-backup/wallets_backup_20251016_130000.json
```

**备份文件格式**:
```json
{
  "backup_time": "20251016_130000",
  "wallets": [
    {
      "wallet_id": "03e0ad6f-9019-450d-9426-26203ab08ef1",
      "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
      "private_key": "0x...",
      "mnemonic": "word1 word2 ... word12",
      "derivation_path": "m/44'/60'/0'/0/0"
    }
  ]
}
```

**优点**:
- ✅ 完整备份所有钱包
- ✅ 可用于生产环境的备份
- ✅ 支持跨环境迁移

**缺点**:
- ⚠️ 明文存储私钥（安全风险）
- ⚠️ 需要手动执行备份和恢复

**安全建议**:
```bash
# 加密备份文件
openssl enc -aes-256-cbc -salt \
  -in ~/.kms-backup/wallets_backup.json \
  -out ~/.kms-backup/wallets_backup.json.enc

# 解密
openssl enc -d -aes-256-cbc \
  -in ~/.kms-backup/wallets_backup.json.enc \
  -out ~/.kms-backup/wallets_backup.json
```

### 3. 自动化工作流

**集成到启动脚本**:

创建 `scripts/kms-auto-start-with-restore.sh`:
```bash
#!/bin/bash
# 启动 KMS 并自动恢复开发钱包

set -e

echo "🚀 启动 KMS..."
./scripts/kms-auto-start.sh

echo ""
echo "⏳ 等待 API Server 启动..."
sleep 15

echo ""
echo "🔄 初始化开发测试钱包..."
./scripts/kms-init-dev-wallets.sh

echo ""
echo "✅ KMS 已启动，测试钱包已就绪！"
echo ""
echo "测试 API:"
echo "  curl http://localhost:3000/health"
echo ""
echo "查看钱包:"
echo "  curl -s -X POST http://localhost:3000/ListKeys \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -H 'x-amz-target: TrentService.ListKeys' \\"
echo "    -d '{}' | jq ."
```

## 使用场景

### 场景 1: 日常开发测试（推荐）

```bash
# 1. 启动 KMS + 初始化测试钱包
./scripts/kms-auto-start.sh && sleep 15 && ./scripts/kms-init-dev-wallets.sh

# 2. 开发测试...

# 3. 重启
pkill qemu  # 或使用 kms-qemu-terminal3.sh
./scripts/kms-auto-start.sh && sleep 15 && ./scripts/kms-init-dev-wallets.sh
```

### 场景 2: 保留重要测试数据

```bash
# 1. 创建了重要的测试钱包，需要备份
./scripts/kms-backup-wallets.sh

# 2. 重启后恢复
./scripts/kms-auto-start.sh
sleep 15
./scripts/kms-restore-wallets.sh ~/.kms-backup/wallets_backup_latest.json
```

### 场景 3: 跨环境迁移

```bash
# 在环境 A 备份
./scripts/kms-backup-wallets.sh

# 复制备份文件到环境 B
scp ~/.kms-backup/wallets_backup_*.json user@host-b:~/.kms-backup/

# 在环境 B 恢复
./scripts/kms-restore-wallets.sh ~/.kms-backup/wallets_backup_*.json
```

## 生产环境方案

⚠️ **以上方案仅适用于开发测试！**

生产环境应该使用：

1. **持久化 Secure Storage**:
   - 配置 OP-TEE 使用持久化存储后端
   - 将 `/data/tee` 挂载到持久化卷
   - 参考: [OP-TEE Secure Storage 文档](https://optee.readthedocs.io/en/latest/architecture/secure_storage.html)

2. **硬件 TEE (Raspberry Pi 5)**:
   - 使用真实硬件，数据自然持久化
   - eMMC/SD 卡存储 Secure Storage

3. **备份策略**:
   - 加密备份到安全位置
   - 使用 HSM (Hardware Security Module)
   - 多重签名 + 社交恢复

## 常见问题

### Q: 为什么固定助记词的钱包 ID 相同？

A: `wallet_id` 是从助记词派生的，相同助记词会生成相同的 `wallet_id`，这正是我们需要的特性。

### Q: 备份文件包含明文私钥，安全吗？

A: 不安全！仅用于开发测试。生产环境必须加密备份或使用硬件 TEE。

### Q: 能否自动定时备份？

A: 可以，使用 cron:
```bash
# 每小时备份一次
0 * * * * cd /path/to/AirAccount && ./scripts/kms-backup-wallets.sh
```

### Q: 如何验证恢复成功？

A: 恢复脚本会自动验证地址是否匹配：
```bash
./scripts/kms-restore-wallets.sh backup.json
# 输出会显示：✅ 地址验证通过
```

## 相关文件

- `scripts/kms-init-dev-wallets.sh` - 初始化固定测试钱包
- `scripts/kms-backup-wallets.sh` - 备份所有钱包
- `scripts/kms-restore-wallets.sh` - 恢复钱包
- `~/.kms-backup/` - 备份文件目录

---

**最后更新**: 2025-10-16
**维护者**: Claude Code
