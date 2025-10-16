#!/bin/bash
# KMS 钱包备份工具 - 导出所有钱包的私钥和助记词

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

BACKUP_DIR="$HOME/.kms-backup"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="$BACKUP_DIR/wallets_backup_$TIMESTAMP.json"

echo -e "${GREEN}🔐 KMS 钱包备份工具${NC}"
echo ""

# 检查 API Server 是否运行
if ! curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo -e "${RED}❌ API Server 未运行${NC}"
    echo "请先启动 API Server: ./scripts/kms-auto-start.sh"
    exit 1
fi

# 创建备份目录
mkdir -p "$BACKUP_DIR"

echo -e "${YELLOW}⚠️  警告：此操作会以明文形式导出私钥！${NC}"
echo "备份文件将保存到: $BACKUP_FILE"
echo ""
read -p "确认继续？(yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "取消备份"
    exit 0
fi

echo ""
echo -e "${GREEN}📋 获取钱包列表...${NC}"

# 获取所有钱包
WALLETS=$(curl -s -X POST http://localhost:3000/ListKeys \
  -H "Content-Type: application/json" \
  -H "x-amz-target: TrentService.ListKeys" \
  -d '{}' | jq -r '.Keys[]?.KeyId // empty')

if [ -z "$WALLETS" ]; then
    echo -e "${YELLOW}⚠️  没有找到钱包${NC}"
    exit 0
fi

WALLET_COUNT=$(echo "$WALLETS" | wc -l | tr -d ' ')
echo -e "${GREEN}找到 $WALLET_COUNT 个钱包${NC}"
echo ""

# 创建备份 JSON
echo "{" > "$BACKUP_FILE"
echo "  \"backup_time\": \"$TIMESTAMP\"," >> "$BACKUP_FILE"
echo "  \"wallets\": [" >> "$BACKUP_FILE"

FIRST=true
for WALLET_ID in $WALLETS; do
    echo -e "${GREEN}导出钱包: $WALLET_ID${NC}"

    # 导出私钥（需要在 Guest VM 中执行）
    EXPORT_SCRIPT="/opt/teaclave/shared/.export_${WALLET_ID}.sh"

    # 创建导出脚本
    docker exec teaclave_dev_env bash -c "cat > $EXPORT_SCRIPT << 'EOF'
#!/bin/sh
cd /root/shared
./export_key $WALLET_ID \"m/44'/60'/0'/0/0\" 2>&1
EOF
chmod +x $EXPORT_SCRIPT"

    # 执行导出
    EXPORT_OUTPUT=$(timeout 5 docker exec teaclave_dev_env bash -c \
      "echo 'sh $EXPORT_SCRIPT' | socat - TCP:localhost:54320" 2>&1 || echo "timeout")

    # 解析输出 (PRIVATE_KEY is for wallet backup, not a hardcoded secret)
    PRIVATE_KEY=$(echo "$EXPORT_OUTPUT" | grep -o "Private key: 0x[0-9a-fA-F]*" | cut -d' ' -f3) # test backup tool
    ADDRESS=$(echo "$EXPORT_OUTPUT" | grep -o "Address: 0x[0-9a-fA-F]*" | cut -d' ' -f2)

    # 获取助记词（从 DescribeKey）
    MNEMONIC=$(curl -s -X POST http://localhost:3000/DescribeKey \
      -H "Content-Type: application/json" \
      -H "x-amz-target: TrentService.DescribeKey" \
      -d "{\"KeyId\":\"$WALLET_ID\"}" | jq -r '.KeyMetadata.Mnemonic // empty')

    if [ -n "$PRIVATE_KEY" ] && [ -n "$ADDRESS" ]; then
        if [ "$FIRST" = false ]; then
            echo "," >> "$BACKUP_FILE"
        fi
        FIRST=false

        # Backup file format (contains private_key for development/test recovery)
        cat >> "$BACKUP_FILE" << EOF
    {
      "wallet_id": "$WALLET_ID",
      "address": "$ADDRESS",
      "private_key": "$PRIVATE_KEY",
      "mnemonic": "$MNEMONIC",
      "derivation_path": "m/44'/60'/0'/0/0"
    }
EOF
        echo -e "  ✅ 地址: $ADDRESS"
    else
        echo -e "  ${YELLOW}⚠️  导出失败${NC}"
    fi

    # 清理临时脚本
    docker exec teaclave_dev_env rm -f "$EXPORT_SCRIPT"
    echo ""
done

echo "  ]" >> "$BACKUP_FILE"
echo "}" >> "$BACKUP_FILE"

echo ""
echo -e "${GREEN}✅ 备份完成！${NC}"
echo ""
echo -e "${YELLOW}备份文件: $BACKUP_FILE${NC}"
echo ""
echo -e "${RED}⚠️  重要：请妥善保管备份文件，包含明文私钥！${NC}"
echo -e "${RED}⚠️  建议加密存储或删除不需要的备份${NC}"
echo ""
echo "查看备份: cat $BACKUP_FILE | jq ."
echo "恢复钱包: ./scripts/kms-restore-wallets.sh $BACKUP_FILE"
