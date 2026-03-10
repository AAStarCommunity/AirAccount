#!/bin/bash
# KMS API Server 测试脚本 - 使用wget（适用于QEMU环境）

echo "🎯 KMS API Server 功能测试 (wget版本)"
echo "================================"

# 检查并启动API服务器
if ! ps | grep -v grep | grep -q kms-api-server; then
    echo "⚠️  API服务器未运行，正在启动..."
    ./kms-api-server > /tmp/kms-api-server.log 2>&1 &
    sleep 3
    echo "✅ API服务器已启动"
else
    echo "✅ API服务器已运行"
fi

echo ""
echo "================================"
echo ""

# 1. 创建密钥
echo "1️⃣ 测试 CreateKey..."
echo '正在发送请求...'
wget -O /tmp/create_response.json --post-data='{"Description":"Test Key","KeyUsage":"SIGN_VERIFY","KeySpec":"ECC_SECG_P256K1","Origin":"AWS_KMS"}' \
  --header='x-amz-target: TrentService.CreateKey' \
  --header='Content-Type: application/x-amz-json-1.1' \
  http://localhost:3000/CreateKey 2>&1

if [ -f /tmp/create_response.json ]; then
    cat /tmp/create_response.json
    echo ""
else
    echo "❌ 请求失败，检查API服务器日志: cat /tmp/kms-api-server.log"
    exit 1
fi

echo ""

# 提取KeyId
KEY_ID=$(cat /tmp/create_response.json | grep -o '"KeyId":"[^"]*"' | head -1 | sed 's/"KeyId":"\([^"]*\)"/\1/')

if [ -n "$KEY_ID" ]; then
    echo "✅ 密钥创建成功: $KEY_ID"
else
    echo "❌ CreateKey 失败"
    exit 1
fi

# 2. 列出密钥
echo ""
echo "2️⃣ 测试 ListKeys..."
wget -qO- --post-data='{}' \
  --header='x-amz-target: TrentService.ListKeys' \
  --header='Content-Type: application/x-amz-json-1.1' \
  http://localhost:3000/ListKeys

echo ""

# 3. 派生地址
echo ""
echo "3️⃣ 测试 DeriveAddress..."
wget -qO- --post-data="{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\"}" \
  --header='x-amz-target: TrentService.DeriveAddress' \
  --header='Content-Type: application/x-amz-json-1.1' \
  http://localhost:3000/DeriveAddress 2>&1 | tee /tmp/derive_response.json

echo ""

if grep -q "Address" /tmp/derive_response.json; then
    ADDRESS=$(cat /tmp/derive_response.json | grep -o '"Address":"[^"]*"' | sed 's/"Address":"\([^"]*\)"/\1/')
    echo "✅ DeriveAddress 成功: $ADDRESS"
else
    echo "❌ DeriveAddress 失败"
fi

# 4. 获取公钥
echo ""
echo "4️⃣ 测试 GetPublicKey..."
wget -qO- --post-data="{\"KeyId\":\"$KEY_ID\"}" \
  --header='x-amz-target: TrentService.GetPublicKey' \
  --header='Content-Type: application/x-amz-json-1.1' \
  http://localhost:3000/GetPublicKey

echo ""

# 5. 签名交易
echo ""
echo "5️⃣ 测试 Sign (签名以太坊交易)..."
wget -qO- --post-data="{\"KeyId\":\"$KEY_ID\",\"DerivationPath\":\"m/44'/60'/0'/0/0\",\"Transaction\":{\"chainId\":1,\"nonce\":0,\"to\":\"0x742d35Cc6634C0532925a3b844Bc454e4438f44e\",\"value\":\"0x0de0b6b3a7640000\",\"gasPrice\":\"0x04a817c800\",\"gas\":21000,\"data\":\"0x\"}}" \
  --header='x-amz-target: TrentService.Sign' \
  --header='Content-Type: application/x-amz-json-1.1' \
  http://localhost:3000/Sign 2>&1 | tee /tmp/sign_response.json

echo ""

if grep -q "Signature" /tmp/sign_response.json; then
    echo "✅ Sign 成功"
else
    echo "❌ Sign 失败"
fi

# 6. 描述密钥
echo ""
echo "6️⃣ 测试 DescribeKey..."
wget -qO- --post-data="{\"KeyId\":\"$KEY_ID\"}" \
  --header='x-amz-target: TrentService.DescribeKey' \
  --header='Content-Type: application/x-amz-json-1.1' \
  http://localhost:3000/DescribeKey

echo ""

# 7. 删除密钥
echo ""
echo "7️⃣ 测试 ScheduleKeyDeletion..."
wget -qO- --post-data="{\"KeyId\":\"$KEY_ID\",\"PendingWindowInDays\":7}" \
  --header='x-amz-target: TrentService.ScheduleKeyDeletion' \
  --header='Content-Type: application/x-amz-json-1.1' \
  http://localhost:3000/DeleteKey

echo ""

# 测试总结
echo ""
echo "================================"
echo "🎉 KMS API 测试完成！"
echo ""
echo "API服务器日志: /tmp/kms-api-server.log"
echo "停止API服务器: killall kms-api-server"
echo ""
echo "清理临时文件:"
echo "  rm /tmp/create_response.json"
echo "  rm /tmp/derive_response.json"
echo "  rm /tmp/sign_response.json"