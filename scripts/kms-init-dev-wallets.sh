#!/bin/bash
# KMS 开发测试钱包初始化 - 创建固定的测试钱包

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🔧 KMS 开发测试钱包初始化${NC}"
echo ""

# 检查 API Server 是否运行
if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo -e "${RED}❌ API Server 未运行${NC}"
    echo "请先启动 API Server: ./scripts/kms-auto-start.sh"
    exit 1
fi

# 固定的测试助记词（仅用于开发测试！）
DEV_MNEMONICS=(
    "test test test test test test test test test test test junk"
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"
)

DEV_NAMES=(
    "dev-wallet-1"
    "dev-wallet-2"
    "dev-wallet-3"
)

echo -e "${YELLOW}⚠️  警告：这些是固定的测试助记词，仅用于开发测试！${NC}"
echo -e "${YELLOW}⚠️  切勿在生产环境使用！${NC}"
echo ""

# 检查是否已经有钱包
EXISTING_COUNT=$(curl -s -X POST http://localhost:3000/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}' | jq '.Keys | length')

if [ "$EXISTING_COUNT" -gt 0 ]; then
    echo -e "${GREEN}当前已有 $EXISTING_COUNT 个钱包${NC}"
    echo ""
    read -p "是否跳过已存在的钱包？(yes/no): " skip_existing
    echo ""
else
    skip_existing="yes"
fi

echo -e "${GREEN}创建开发测试钱包...${NC}"
echo ""

for i in "${!DEV_MNEMONICS[@]}"; do
    MNEMONIC="${DEV_MNEMONICS[$i]}"
    NAME="${DEV_NAMES[$i]}"

    echo -e "${BLUE}[$((i + 1))/${#DEV_MNEMONICS[@]}] 创建: $NAME${NC}"

    # 创建钱包
    RESULT=$(curl -s -X POST http://localhost:3000/CreateKey \
      -H "Content-Type: application/json" \
      -H "x-amz-target: TrentService.CreateKey" \
      -d "{\"Mnemonic\":\"$MNEMONIC\"}")

    WALLET_ID=$(echo "$RESULT" | jq -r '.KeyMetadata.KeyId // empty')

    if [ -n "$WALLET_ID" ]; then
        echo -e "  ${GREEN}✅ 钱包 ID: $WALLET_ID${NC}"

        # 获取默认地址
        ADDRESS=$(curl -s -X POST http://localhost:3000/DeriveAddress \
          -H "Content-Type: application/json" \
          -H "x-amz-target: TrentService.DeriveAddress" \
          -d "{\"KeyId\":\"$WALLET_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\"}" \
          | jq -r '.Address // empty')

        if [ -n "$ADDRESS" ]; then
            echo -e "  ${GREEN}✅ 地址: $ADDRESS${NC}"
        fi
    else
        ERROR=$(echo "$RESULT" | jq -r '.message // .error // "Unknown error"')
        if echo "$ERROR" | grep -q "already exists"; then
            echo -e "  ${YELLOW}⚠️  钱包已存在（助记词相同会生成相同的 wallet_id）${NC}"
        else
            echo -e "  ${RED}❌ 创建失败: $ERROR${NC}"
        fi
    fi
    echo ""
done

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}初始化完成！${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "查看所有钱包:"
echo "  curl -s -X POST http://localhost:3000/ListKeys \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -H 'x-amz-target: TrentService.ListKeys' \\"
echo "    -d '{}' | jq ."
echo ""
echo -e "${YELLOW}💡 提示：${NC}"
echo "  - 这些测试钱包会在每次 QEMU 重启后自动恢复（因为使用固定助记词）"
echo "  - 如果需要测试其他功能，可以创建新钱包"
echo "  - 备份所有钱包: ./scripts/kms-backup-wallets.sh"
echo "  - 恢复钱包: ./scripts/kms-restore-wallets.sh <backup-file>"
