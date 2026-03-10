#!/bin/bash
# KMS QEMU测试脚本 - 在Guest VM中运行

echo "🎯 KMS功能测试"
echo "================================"

# 1. 创建钱包
echo ""
echo "1️⃣ 创建钱包..."
WALLET_OUTPUT=$(./kms create-wallet 2>&1)
echo "$WALLET_OUTPUT"
WALLET_ID=$(echo "$WALLET_OUTPUT" | grep "Wallet ID:" | awk '{print $3}')

if [ -z "$WALLET_ID" ]; then
    echo "❌ 创建钱包失败"
    exit 1
fi

echo "✅ 钱包创建成功: $WALLET_ID"

# 2. 派生地址
echo ""
echo "2️⃣ 派生以太坊地址 (m/44'/60'/0'/0/0)..."
ADDRESS_OUTPUT=$(./kms derive-address $WALLET_ID "m/44'/60'/0'/0/0" 2>&1)
echo "$ADDRESS_OUTPUT"
ADDRESS=$(echo "$ADDRESS_OUTPUT" | grep "Address:" | awk '{print $2}')

if [ -z "$ADDRESS" ]; then
    echo "❌ 派生地址失败"
else
    echo "✅ 地址派生成功: $ADDRESS"
fi

# 3. 签名交易
echo ""
echo "3️⃣ 签名以太坊交易..."
SIGN_OUTPUT=$(./kms sign-transaction $WALLET_ID "m/44'/60'/0'/0/0" \
  --chain-id 1 \
  --nonce 0 \
  --to 0x742d35Cc6634C0532925a3b844Bc454e4438f44e \
  --value 1000000000000000000 \
  --gas-price 20000000000 \
  --gas 21000 2>&1)
echo "$SIGN_OUTPUT"

if echo "$SIGN_OUTPUT" | grep -q "Signature:"; then
    echo "✅ 交易签名成功"
else
    echo "❌ 交易签名失败"
fi

# 4. 删除钱包
echo ""
echo "4️⃣ 删除钱包..."
./kms remove-wallet $WALLET_ID 2>&1

if [ $? -eq 0 ]; then
    echo "✅ 钱包删除成功"
else
    echo "❌ 钱包删除失败"
fi

echo ""
echo "🎉 测试完成！"
