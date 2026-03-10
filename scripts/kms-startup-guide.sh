#!/bin/bash
# KMS 系统启动指南

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}═══════════════════════════════════════════${NC}"
echo -e "${BLUE}     KMS 系统完整启动指南${NC}"
echo -e "${BLUE}═══════════════════════════════════════════${NC}"
echo ""

echo -e "${YELLOW}📋 启动前检查清单：${NC}"
echo ""

# 1. 检查 Docker
echo -n "  1. Docker 容器状态: "
if docker ps | grep -q teaclave_dev_env; then
    echo -e "${GREEN}✓ 运行中${NC}"
else
    echo -e "${YELLOW}✗ 未运行${NC}"
    echo -e "     ${YELLOW}→ 运行: docker start teaclave_dev_env${NC}"
fi

# 2. 检查 Cloudflared
echo -n "  2. Cloudflared 隧道: "
if ps aux | grep -v grep | grep -q "cloudflared tunnel run"; then
    echo -e "${GREEN}✓ 运行中${NC}"
else
    echo -e "${YELLOW}✗ 未运行${NC}"
    echo -e "     ${YELLOW}→ 运行: cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &${NC}"
fi

# 3. 检查 QEMU
echo -n "  3. QEMU 虚拟机: "
if docker exec teaclave_dev_env ps aux 2>/dev/null | grep -q "[q]emu-system-aarch64"; then
    echo -e "${GREEN}✓ 运行中${NC}"
else
    echo -e "${YELLOW}✗ 未运行${NC}"
fi

# 4. 检查 API Server
echo -n "  4. KMS API Server: "
if curl -s -m 2 http://localhost:3000/health > /dev/null 2>&1; then
    echo -e "${GREEN}✓ 运行中${NC}"
else
    echo -e "${YELLOW}✗ 未运行${NC}"
fi

echo ""
echo -e "${BLUE}═══════════════════════════════════════════${NC}"
echo -e "${YELLOW}🚀 启动方式选择：${NC}"
echo ""
echo -e "${GREEN}方式 A: 一键自动启动${NC}（推荐，快速启动）"
echo -e "  命令: ${BLUE}./scripts/kms-auto-start.sh${NC}"
echo -e "  优点: 一条命令启动所有服务"
echo -e "  缺点: 后台运行，无法实时查看日志"
echo -e "  监控: 使用 ${BLUE}./scripts/kms-monitor.sh${NC} 查看日志"
echo ""
echo -e "${GREEN}方式 B: 手动三终端启动${NC}（用于调试，可实时查看日志）"
echo -e "  终端 1: ${BLUE}./scripts/terminal3-secure-log.sh${NC}  (TA 日志)"
echo -e "  终端 2: ${BLUE}./scripts/terminal2-guest-vm.sh${NC}   (CA 日志)"
echo -e "  终端 3: ${BLUE}./scripts/terminal1-qemu.sh${NC}       (QEMU + API)"
echo -e "  优点: 实时查看所有日志"
echo -e "  缺点: 需要三个终端窗口"
echo ""
echo -e "${BLUE}═══════════════════════════════════════════${NC}"
echo -e "${YELLOW}🔧 其他常用命令：${NC}"
echo ""
echo -e "  • 部署新代码: ${BLUE}./scripts/kms-deploy.sh${NC}"
echo -e "  • 重启 API:   ${BLUE}./scripts/kms-restart-api.sh${NC}"
echo -e "  • 查看日志:   ${BLUE}./scripts/kms-monitor.sh${NC}"
echo ""
echo -e "${BLUE}═══════════════════════════════════════════${NC}"
echo -e "${YELLOW}📡 访问地址：${NC}"
echo ""
echo -e "  • 本地:  ${GREEN}http://localhost:3000${NC}"
echo -e "  • 公网:  ${GREEN}https://kms.aastar.io${NC}"
echo -e "  • 健康检查: ${BLUE}curl http://localhost:3000/health${NC}"
echo ""
echo -e "${BLUE}═══════════════════════════════════════════${NC}"
echo -e "${YELLOW}⚙️  完整启动流程（首次或 Mac 重启后）：${NC}"
echo ""
echo -e "  1. ${BLUE}docker start teaclave_dev_env${NC}"
echo -e "  2. ${BLUE}cloudflared tunnel run kms-tunnel > /tmp/cloudflared.log 2>&1 &${NC}"
echo -e "  3. ${BLUE}./scripts/kms-auto-start.sh${NC}  或  手动启动三个终端"
echo ""
echo -e "${BLUE}═══════════════════════════════════════════${NC}"
