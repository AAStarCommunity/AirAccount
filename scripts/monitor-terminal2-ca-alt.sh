#!/bin/bash
# Terminal 2: KMS API Server (CA) 监控 - 替代方案
# 通过 Cloudflared 日志监控 API 调用（更稳定）

echo "🔐 Terminal 2: KMS API Server (CA) 监控"
echo "=================================================="
echo ""
echo "功能："
echo "  - 监控 HTTP API 请求"
echo "  - 显示 API 端点调用"
echo "  - 请求响应状态"
echo ""
echo "⚠️  提示："
echo "   由于 socat 在 tmux 中不稳定，此脚本从 Cloudflared 日志"
echo "   提取 API 调用信息作为 CA 层监控的替代方案"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 验证服务状态
echo "📊 验证 KMS API Server 状态..."
SERVICE_STATUS=$(curl -s https://kms.aastar.io/health 2>&1)
if echo "$SERVICE_STATUS" | grep -q "healthy"; then
    echo "✅ KMS API Server: 运行中"
    echo "   Status: $(echo "$SERVICE_STATUS" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)"
    echo "   TA Mode: $(echo "$SERVICE_STATUS" | grep -o '"ta_mode":"[^"]*"' | cut -d'"' -f4)"
    echo "   Version: $(echo "$SERVICE_STATUS" | grep -o '"version":"[^"]*"' | cut -d'"' -f4)"
else
    echo "❌ KMS API Server 可能未运行"
    echo "   $SERVICE_STATUS"
fi

echo ""
echo "📝 实时 API 调用监控:"
echo "=================================================="
echo ""

# 监控 cloudflared 日志，提取并格式化 API 调用
docker exec -it teaclave_dev_env bash -c '
tail -f /tmp/cloudflared.log | grep --line-buffered "DBG" | while IFS= read -r line; do
    # 提取时间戳
    TIMESTAMP=$(echo "$line" | grep -oE "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z")

    # 检查是否是请求行
    if echo "$line" | grep -qE "(GET|POST) https://"; then
        METHOD=$(echo "$line" | grep -oE "(GET|POST)")
        PATH=$(echo "$line" | grep -oE "path=/[^ ]+" | cut -d"=" -f2 | cut -d" " -f1)

        # 显示请求
        if [ -n "$PATH" ]; then
            printf "\n[%s] 📨 %s %s\n" "$TIMESTAMP" "$METHOD" "$PATH"

            # 根据端点显示说明
            case "$PATH" in
                */CreateKey)
                    echo "   └─ 正在调用 TA: 创建新钱包"
                    ;;
                */DescribeKey)
                    echo "   └─ 正在调用 TA: 查询密钥元数据"
                    ;;
                */ListKeys)
                    echo "   └─ 正在调用 TA: 列出所有密钥"
                    ;;
                */GetPublicKey)
                    echo "   └─ 正在调用 TA: 获取公钥"
                    ;;
                */DeriveAddress)
                    echo "   └─ 正在调用 TA: 派生以太坊地址"
                    ;;
                */Sign)
                    echo "   └─ 正在调用 TA: 签名消息"
                    ;;
                */DeleteKey)
                    echo "   └─ 正在调用 TA: 删除密钥"
                    ;;
                */health)
                    echo "   └─ 健康检查 (不调用 TA)"
                    ;;
            esac
        fi
    fi

    # 检查是否是响应行
    if echo "$line" | grep -qE "(200 OK|201|400|404|500)"; then
        STATUS=$(echo "$line" | grep -oE "(200 OK|201|400 [^\"]+|404|500 [^\"]+)" | head -1)
        CONTENT_LENGTH=$(echo "$line" | grep -oE "content-length=[0-9]+" | cut -d"=" -f2)

        if [ -n "$STATUS" ]; then
            printf "   ✅ 响应: %s" "$STATUS"
            if [ -n "$CONTENT_LENGTH" ]; then
                printf " (size: %s bytes)" "$CONTENT_LENGTH"
            fi
            printf "\n"
        fi
    fi
done
'