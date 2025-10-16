#!/bin/bash
# KMS 钱包恢复工具 - 从共享目录备份文件恢复钱包

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}🔄 KMS 钱包恢复工具${NC}"
echo ""

# 备份目录（Guest VM 和 Host 都可访问）
BACKUP_DIR="/root/shared/kms-wallets-backup"

# 检查参数
if [ -z "$1" ]; then
    echo "用法: $0 <备份文件路径>"
    echo ""
    echo "示例:"
    echo "  $0 $BACKUP_DIR/wallet_4a0581c5-4522-49b3-bf44-4a7d9d1290ba.json"
    echo ""
    echo -e "${YELLOW}可用备份:${NC}"
    if [ -d "$BACKUP_DIR" ] && [ "$(ls -A $BACKUP_DIR/*.json 2>/dev/null)" ]; then
        ls -lh $BACKUP_DIR/*.json 2>/dev/null || echo "  (暂无备份)"
    else
        echo "  (暂无备份)"
    fi
    exit 1
fi

BACKUP_FILE="$1"

if [ ! -f "$BACKUP_FILE" ]; then
    echo -e "${RED}❌ 备份文件不存在: $BACKUP_FILE${NC}"
    exit 1
fi

# 检查 API Server 是否运行
if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo -e "${RED}❌ API Server 未运行${NC}"
    echo "请先启动 API Server: ./scripts/kms-auto-start.sh"
    exit 1
fi

echo -e "${GREEN}📂 读取备份文件: $BACKUP_FILE${NC}"
echo ""

# 读取备份信息
WALLET_ID=$(jq -r '.wallet_id' "$BACKUP_FILE")
ADDRESS=$(jq -r '.address' "$BACKUP_FILE")
MNEMONIC=$(jq -r '.mnemonic' "$BACKUP_FILE")
DERIVATION_PATH=$(jq -r '.derivation_path // "m/44'"'"'/60'"'"'/0'"'"'/0/0"' "$BACKUP_FILE")
BACKUP_TIME=$(jq -r '.backup_time // "unknown"' "$BACKUP_FILE")

echo "钱包 ID: $WALLET_ID"
echo "地址: $ADDRESS"
echo "派生路径: $DERIVATION_PATH"
echo "备份时间: $BACKUP_TIME"
echo ""

if [ -z "$MNEMONIC" ] || [ "$MNEMONIC" = "null" ]; then
    echo -e "${RED}❌ 备份文件中缺少助记词，无法恢复${NC}"
    exit 1
fi

# 确认恢复
read -p "确认恢复此钱包？(yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "取消恢复"
    exit 0
fi

echo ""
echo -e "${GREEN}🔄 开始恢复钱包...${NC}"
echo ""

# 检查钱包是否已存在
EXISTING=$(curl -s -X POST http://localhost:3000/DescribeKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.DescribeKey" \
  -d "{\"KeyId\":\"$WALLET_ID\"}" 2>&1)

if echo "$EXISTING" | jq -e '.KeyMetadata.KeyId' > /dev/null 2>&1; then
    echo -e "${YELLOW}⚠️  钱包已存在，跳过恢复${NC}"
    echo ""
    EXISTING_ADDR=$(echo "$EXISTING" | jq -r '.KeyMetadata.Address // "unknown"')
    echo "现有地址: $EXISTING_ADDR"

    if [ "$EXISTING_ADDR" = "$ADDRESS" ]; then
        echo -e "${GREEN}✅ 地址验证通过${NC}"
    else
        echo -e "${RED}⚠️  地址不匹配！备份: $ADDRESS, 现有: $EXISTING_ADDR${NC}"
    fi
    exit 0
fi

# 使用助记词创建钱包（会生成相同的 wallet_id）
RESULT=$(curl -s -X POST http://localhost:3000/CreateKey \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.CreateKey" \
  -d "{
    \"KeyId\":\"$WALLET_ID\",
    \"Mnemonic\":\"$MNEMONIC\",
    \"Description\":\"Restored wallet $WALLET_ID\",
    \"KeyUsage\":\"SIGN_VERIFY\",
    \"KeySpec\":\"ECC_SECG_P256K1\",
    \"Origin\":\"AWS_KMS\"
  }")

NEW_ID=$(echo "$RESULT" | jq -r '.KeyMetadata.KeyId // empty')

if [ -n "$NEW_ID" ]; then
    echo -e "${GREEN}✅ 钱包恢复成功！${NC}"
    echo ""
    echo "钱包 ID: $NEW_ID"

    # 验证地址是否匹配
    NEW_ADDR=$(echo "$RESULT" | jq -r '.KeyMetadata.Address // empty')

    if [ "$NEW_ADDR" = "$ADDRESS" ]; then
        echo -e "${GREEN}✅ 地址验证通过: $NEW_ADDR${NC}"
    else
        echo -e "${RED}⚠️  地址不匹配！期望: $ADDRESS, 实际: $NEW_ADDR${NC}"
    fi
else
    echo -e "${RED}❌ 恢复失败${NC}"
    echo "错误: $RESULT"
    exit 1
fi

echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}✅ 钱包恢复完成！${NC}"
echo -e "${BLUE}========================================${NC}"
