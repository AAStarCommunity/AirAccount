#!/bin/bash
# KMS 钱包恢复工具 - 从备份文件恢复钱包

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}🔄 KMS 钱包恢复工具${NC}"
echo ""

# 检查参数
if [ -z "$1" ]; then
    echo "用法: $0 <备份文件路径>"
    echo ""
    echo "示例:"
    echo "  $0 ~/.kms-backup/wallets_backup_20251016_130000.json"
    echo ""
    echo "可用备份:"
    ls -lh ~/.kms-backup/wallets_backup_*.json 2>/dev/null || echo "  (暂无备份)"
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
BACKUP_TIME=$(jq -r '.backup_time' "$BACKUP_FILE")
WALLET_COUNT=$(jq '.wallets | length' "$BACKUP_FILE")

echo "备份时间: $BACKUP_TIME"
echo "钱包数量: $WALLET_COUNT"
echo ""

if [ "$WALLET_COUNT" -eq 0 ]; then
    echo -e "${YELLOW}⚠️  备份文件中没有钱包${NC}"
    exit 0
fi

# 显示钱包列表
echo -e "${GREEN}钱包列表:${NC}"
jq -r '.wallets[] | "  - \(.wallet_id) (\(.address))"' "$BACKUP_FILE"
echo ""

read -p "确认恢复这些钱包？(yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "取消恢复"
    exit 0
fi

echo ""
echo -e "${GREEN}🔄 开始恢复钱包...${NC}"
echo ""

SUCCESS_COUNT=0
FAIL_COUNT=0

# 逐个恢复钱包
for i in $(seq 0 $((WALLET_COUNT - 1))); do
    WALLET_ID=$(jq -r ".wallets[$i].wallet_id" "$BACKUP_FILE")
    ADDRESS=$(jq -r ".wallets[$i].address" "$BACKUP_FILE")
    MNEMONIC=$(jq -r ".wallets[$i].mnemonic" "$BACKUP_FILE")

    echo -e "${YELLOW}恢复钱包 $((i + 1))/$WALLET_COUNT: $WALLET_ID${NC}"

    # 检查钱包是否已存在
    EXISTING=$(curl -s -X POST http://localhost:3000/DescribeKey \
      -H "Content-Type: application/json" \
      -H "x-amz-target: TrentService.DescribeKey" \
      -d "{\"KeyId\":\"$WALLET_ID\"}" 2>&1)

    if echo "$EXISTING" | jq -e '.KeyMetadata.KeyId' > /dev/null 2>&1; then
        echo -e "  ${YELLOW}⚠️  钱包已存在，跳过${NC}"
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    else
        # 使用助记词创建钱包（会生成相同的 wallet_id）
        if [ -n "$MNEMONIC" ] && [ "$MNEMONIC" != "null" ]; then
            RESULT=$(curl -s -X POST http://localhost:3000/CreateKey \
              -H "Content-Type: application/json" \
              -H "x-amz-target: TrentService.CreateKey" \
              -d "{\"Mnemonic\":\"$MNEMONIC\"}")

            NEW_ID=$(echo "$RESULT" | jq -r '.KeyMetadata.KeyId // empty')

            if [ -n "$NEW_ID" ]; then
                echo -e "  ${GREEN}✅ 恢复成功 (ID: $NEW_ID)${NC}"
                SUCCESS_COUNT=$((SUCCESS_COUNT + 1))

                # 验证地址是否匹配
                NEW_ADDR=$(curl -s -X POST http://localhost:3000/DeriveAddress \
                  -H "Content-Type: application/json" \
                  -H "x-amz-target: TrentService.DeriveAddress" \
                  -d "{\"KeyId\":\"$NEW_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\"}" \
                  | jq -r '.Address // empty')

                if [ "$NEW_ADDR" = "$ADDRESS" ]; then
                    echo -e "  ${GREEN}✅ 地址验证通过: $NEW_ADDR${NC}"
                else
                    echo -e "  ${RED}⚠️  地址不匹配！期望: $ADDRESS, 实际: $NEW_ADDR${NC}"
                fi
            else
                echo -e "  ${RED}❌ 恢复失败${NC}"
                echo "  错误: $RESULT"
                FAIL_COUNT=$((FAIL_COUNT + 1))
            fi
        else
            echo -e "  ${RED}❌ 缺少助记词，无法恢复${NC}"
            FAIL_COUNT=$((FAIL_COUNT + 1))
        fi
    fi
    echo ""
done

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}恢复完成！${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "成功: $SUCCESS_COUNT"
echo "失败: $FAIL_COUNT"
echo "总计: $WALLET_COUNT"
echo ""

if [ $SUCCESS_COUNT -eq $WALLET_COUNT ]; then
    echo -e "${GREEN}✅ 所有钱包恢复成功！${NC}"
else
    echo -e "${YELLOW}⚠️  部分钱包恢复失败，请检查日志${NC}"
fi
