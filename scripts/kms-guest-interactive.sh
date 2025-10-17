#!/bin/bash
# Guest VM 交互式命令执行工具 - 适配 v2 启动模式

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  KMS Guest VM 交互式工具${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# 检查 shared 目录是否可访问
if docker exec teaclave_dev_env test -d /opt/teaclave/shared; then
    echo -e "${GREEN}✅ Shared 目录可访问${NC}"
else
    echo -e "${YELLOW}❌ Shared 目录不可访问${NC}"
    exit 1
fi

echo ""
echo -e "${GREEN}📝 可用操作:${NC}"
echo "  1. 查看 shared 目录文件"
echo "  2. 检查 QEMU Guest VM 状态"
echo "  3. 启动 API Server"
echo "  4. 停止 API Server"
echo "  5. 检查 API Server 状态"
echo "  6. 列出钱包 (需要 API Server 运行)"
echo "  7. 执行自定义命令"
echo "  8. 部署新的 TA 二进制"
echo "  0. 退出"
echo ""

while true; do
    echo -e -n "${BLUE}guest-vm>${NC} "
    read -p "" choice

    case $choice in
        1)
            echo ""
            echo -e "${GREEN}📂 Shared 目录内容:${NC}"
            docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/
            echo ""
            ;;
        2)
            echo ""
            echo -e "${GREEN}🔍 Guest VM 状态:${NC}"
            echo "发送 'uname -a' 命令到 Guest VM..."
            echo 'uname -a' | docker exec -i teaclave_dev_env timeout 3 socat - TCP:localhost:54320 2>&1 || echo "(无响应或超时)"
            echo ""
            ;;
        3)
            echo ""
            echo -e "${GREEN}🚀 启动 API Server...${NC}"
            echo "执行命令: cd /root/shared && nohup ./kms_ca > api.log 2>&1 &"
            echo 'cd /root/shared && nohup ./kms_ca > api.log 2>&1 &' | docker exec -i teaclave_dev_env socat - TCP:localhost:54320 2>&1
            sleep 2
            echo ""
            echo -e "${YELLOW}⏳ 等待 15 秒让 API Server 启动...${NC}"
            sleep 15
            echo ""
            echo -e "${GREEN}测试 API Server...${NC}"
            curl -s http://localhost:3000/health | jq . 2>/dev/null || echo "API Server 未响应，可能需要更多时间启动"
            echo ""
            ;;
        4)
            echo ""
            echo -e "${YELLOW}🛑 停止 API Server...${NC}"
            echo 'pkill -f kms_ca' | docker exec -i teaclave_dev_env socat - TCP:localhost:54320 2>&1
            sleep 1
            echo -e "${GREEN}✅ 已发送停止命令${NC}"
            echo ""
            ;;
        5)
            echo ""
            echo -e "${GREEN}🔍 API Server 状态:${NC}"
            curl -s http://localhost:3000/health | jq . 2>/dev/null || echo "❌ API Server 未运行"
            echo ""
            ;;
        6)
            echo ""
            echo -e "${GREEN}🔑 列出钱包:${NC}"
            curl -s -X POST http://localhost:3000/ListKeys \
              -H "Content-Type: application/json" \
              -H "x-amz-target: TrentService.ListKeys" \
              -d '{}' | jq . 2>/dev/null || echo "❌ API Server 未运行或请求失败"
            echo ""
            ;;
        7)
            echo ""
            read -p "输入要执行的命令: " cmd
            if [ -n "$cmd" ]; then
                echo -e "${GREEN}执行: $cmd${NC}"
                echo "$cmd" | docker exec -i teaclave_dev_env timeout 5 socat - TCP:localhost:54320 2>&1 || echo "(无响应或超时)"
            fi
            echo ""
            ;;
        8)
            echo ""
            echo -e "${GREEN}🚀 部署新的 TA 二进制...${NC}"
            docker exec teaclave_dev_env bash -c "
                if [ -f /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta ]; then
                    cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/teaclave/shared/ta/
                    echo '✅ TA 二进制已部署'
                    ls -lh /opt/teaclave/shared/ta/*.ta
                else
                    echo '❌ TA 二进制未找到，需要先编译'
                fi
            "
            echo ""
            ;;
        0)
            echo "再见！"
            exit 0
            ;;
        *)
            echo -e "${YELLOW}无效选项${NC}"
            ;;
    esac
done
