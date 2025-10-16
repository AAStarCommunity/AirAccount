#!/bin/bash
# KMS QEMU - Terminal 2: Guest VM监听器 + 自动初始化测试钱包
# 必须在Terminal 1 (QEMU) 之前启动

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "🔌 启动 Guest VM 监听器 (端口 54320)..."
echo "   (会自动登录、挂载、启动 API Server)"
echo ""

# 在后台启动监听器（移除 -it，因为后台运行不需要 TTY）
docker exec -d teaclave_dev_env bash -l -c "listen_on_guest_vm_shell"

echo ""
echo "⏳ 等待 API Server 启动..."
echo "   (最多等待 60 秒)"
echo ""

# 等待 API Server 就绪
for i in {1..60}; do
    if curl -s http://localhost:3000/health > /dev/null 2>&1; then
        echo ""
        echo "✅ API Server 已就绪"
        break
    fi
    sleep 1
    if [ $((i % 10)) -eq 0 ]; then
        echo -n "."
    fi
done

# 检查 API Server 是否成功启动
if curl -s http://localhost:3000/health > /dev/null 2>&1; then
    echo ""
    echo "🔄 初始化开发测试钱包..."
    "$SCRIPT_DIR/kms-init-dev-wallets.sh"

    echo ""
    echo "========================================"
    echo "✅ Guest VM 已启动，测试钱包已就绪！"
    echo "========================================"
    echo ""
    echo "📊 快速测试:"
    echo "  curl http://localhost:3000/health"
    echo ""
    echo "📋 查看钱包:"
    echo "  curl -s -X POST http://localhost:3000/ListKeys \\"
    echo "    -H 'Content-Type: application/json' \\"
    echo "    -H 'x-amz-target: TrentService.ListKeys' \\"
    echo "    -d '{}' | jq ."
    echo ""
else
    echo ""
    echo "⚠️  API Server 未启动，跳过钱包初始化"
    echo "可以稍后手动运行: ./scripts/kms-init-dev-wallets.sh"
    echo ""
fi

echo "💡 提示:"
echo "  - Guest VM 监听器在后台运行"
echo "  - 查看日志: docker exec teaclave_dev_env ps aux | grep listen_on_guest_vm_shell"
echo "  - 停止服务: docker exec teaclave_dev_env pkill -f listen_on_guest_vm_shell"
echo ""
echo "✅ Terminal 2 初始化完成"
echo ""
