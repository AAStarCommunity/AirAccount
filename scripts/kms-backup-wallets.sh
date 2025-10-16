#!/bin/bash
# KMS 钱包备份工具 - 将共享目录备份复制到用户目录

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

BACKUP_DIR_SOURCE="/root/shared/kms-wallets-backup"
BACKUP_DIR_TARGET="$HOME/.kms-backup"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

echo -e "${GREEN}🔐 KMS 钱包备份工具${NC}"
echo ""

# 检查源备份目录
if [ ! -d "$BACKUP_DIR_SOURCE" ] || [ -z "$(ls -A $BACKUP_DIR_SOURCE/*.json 2>/dev/null)" ]; then
    echo -e "${YELLOW}⚠️  共享目录中没有备份文件${NC}"
    echo "备份文件在钱包创建时自动生成（需要提供助记词参数）"
    echo "位置: $BACKUP_DIR_SOURCE"
    exit 0
fi

echo -e "${GREEN}📋 发现备份文件:${NC}"
ls -lh $BACKUP_DIR_SOURCE/*.json
echo ""

WALLET_COUNT=$(ls -1 $BACKUP_DIR_SOURCE/*.json 2>/dev/null | wc -l | tr -d ' ')
echo -e "${GREEN}共 $WALLET_COUNT 个钱包备份${NC}"
echo ""

# 仅在交互模式下询问确认
if [ -t 0 ]; then
    read -p "确认复制到用户目录？(yes/no): " confirm
    if [ "$confirm" != "yes" ]; then
        echo "取消备份"
        exit 0
    fi
else
    # 非交互模式（cron）自动继续
    echo "ℹ️  非交互模式，自动执行备份..."
fi

echo ""

# 创建目标目录
mkdir -p "$BACKUP_DIR_TARGET"

# 复制备份文件
echo -e "${GREEN}📦 复制备份文件...${NC}"
cp -v $BACKUP_DIR_SOURCE/*.json "$BACKUP_DIR_TARGET/"

echo ""
echo -e "${GREEN}✅ 备份完成！${NC}"
echo ""
echo -e "${YELLOW}备份位置: $BACKUP_DIR_TARGET${NC}"
echo ""
echo -e "${RED}⚠️  重要：请妥善保管备份文件，包含明文助记词！${NC}"
echo -e "${RED}⚠️  建议加密存储或删除不需要的备份${NC}"
echo ""
echo "查看备份: ls -lh $BACKUP_DIR_TARGET/"
echo "恢复钱包: ./scripts/kms-restore-wallets.sh $BACKUP_DIR_SOURCE/wallet_<ID>.json"
