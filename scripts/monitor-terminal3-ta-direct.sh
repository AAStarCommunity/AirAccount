#!/bin/bash
# Terminal 3: OP-TEE TA (Secure World) 监控 - 真实日志
# 通过定期查询 dmesg 显示 OP-TEE 相关日志

echo "🔒 Terminal 3: OP-TEE TA (Secure World) 监控 (真实日志)"
echo "=================================================="
echo ""
echo "功能："
echo "  - 显示 OP-TEE 内核日志"
echo "  - TA 会话和命令调用"
echo "  - Secure World 操作记录"
echo ""
echo "TA UUID: 4319f351-0b24-4097-b659-80ee4f824cdd"
echo ""
echo "日志来源: dmesg (QEMU Guest 内核日志)"
echo ""
echo "⚠️  注意："
echo "   OP-TEE TA 的详细日志需要在编译时启用 trace"
echo "   当前显示的是系统级别的 OP-TEE 框架日志"
echo ""
echo "开始监控..."
echo "=================================================="
echo ""

# 记录上次显示的行数
LAST_LINE=0

while true; do
    # 获取当前的 OP-TEE 相关日志
    CURRENT_LOGS=$(docker exec teaclave_dev_env bash -c "
        timeout 5 bash -c '
        (
        echo \"dmesg | grep -i -E \\\"(optee|teec|tee)\\\" | tail -50\"
        sleep 2
        ) | socat - TCP:localhost:54320 2>&1
        ' 2>&1
    " | grep -E "optee|teec|tee" | grep -v "grep")

    # 计算新增的行数
    CURRENT_LINE=$(echo "$CURRENT_LOGS" | wc -l)

    if [ "$CURRENT_LINE" -gt "$LAST_LINE" ]; then
        # 只显示新增的日志
        echo "$CURRENT_LOGS" | tail -n +$((LAST_LINE + 1))
        LAST_LINE=$CURRENT_LINE
    fi

    # 每 3 秒刷新一次
    sleep 3

    # 如果 socat 失败，尝试另一种方式
    if [ -z "$CURRENT_LOGS" ]; then
        echo "[$(date '+%H:%M:%S')] ⚠️  无法获取 OP-TEE 日志，3秒后重试..."
        echo ""
        echo "💡 提示: 你也可以在单独的终端中运行："
        echo "   socat - TCP:localhost:54320"
        echo "   然后执行: dmesg | grep -i optee | tail -30"
        echo ""
        sleep 3
    fi
done