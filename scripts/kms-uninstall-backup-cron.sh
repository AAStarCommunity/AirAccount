#!/bin/bash
# KMS 自动备份 Cron 任务卸载脚本

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}🗑️  KMS 钱包自动备份卸载程序${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 检查是否存在 cron 任务
EXISTING_CRON=$(crontab -l 2>/dev/null | grep -F "kms-backup-wallets.sh" || true)

if [ -z "$EXISTING_CRON" ]; then
    echo -e "${YELLOW}⚠️  未找到 KMS 备份 cron 任务${NC}"
    echo ""
    echo "当前所有 cron 任务:"
    crontab -l 2>/dev/null || echo "  (无)"
    exit 0
fi

echo -e "${YELLOW}发现以下 KMS 备份任务:${NC}"
echo "  $EXISTING_CRON"
echo ""

# 确认卸载
if [ "$1" != "--force" ] && [ -t 0 ]; then
    echo -e "${RED}确认要移除此 cron 任务吗? (yes/no):${NC} "
    read -r CONFIRM
    if [ "$CONFIRM" != "yes" ]; then
        echo "取消卸载"
        exit 0
    fi
fi

# 移除 cron 任务
echo -e "${GREEN}[INFO]${NC} 正在移除 cron 任务..."
crontab -l 2>/dev/null | grep -v "kms-backup-wallets.sh" | crontab -

echo -e "${GREEN}✅ Cron 任务已成功移除!${NC}"
echo ""

# 验证
echo "当前剩余的 cron 任务:"
crontab -l 2>/dev/null || echo "  (无)"
echo ""

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}📊 说明${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "✅ 自动备份已停止"
echo "📁 历史备份文件仍保留在 ~/.kms-backup/"
echo ""
echo "如需重新启用自动备份，请运行:"
echo "  ./scripts/kms-install-backup-cron.sh"
echo ""
