#!/bin/bash

# AirAccount SDK-CA 快速连接测试
# 专门测试SDK模拟器到CA服务的基本连接

echo "🔌 AirAccount SDK-CA 快速连接测试"
echo "=================================="

# 检查CA服务是否运行
check_ca_service() {
    local ca_type=$1
    local port=$2
    
    echo "检查 ${ca_type} CA 服务 (端口 ${port})..."
    
    if curl -s --max-time 3 "http://localhost:${port}/health" > /dev/null; then
        echo "✅ ${ca_type} CA 服务正常"
        return 0
    else
        echo "❌ ${ca_type} CA 服务未响应"
        return 1
    fi
}

# 快速API测试
quick_api_test() {
    local ca_type=$1
    local port=$2
    
    echo "测试 ${ca_type} CA API 端点..."
    
    # 健康检查
    HEALTH=$(curl -s "http://localhost:${port}/health")
    if echo "$HEALTH" | grep -q '"status":"healthy"\|"tee_connected":true'; then
        echo "  ✅ 健康检查通过"
    else
        echo "  ❌ 健康检查失败"
        echo "$HEALTH"
        return 1
    fi
    
    # WebAuthn端点测试（不需要真实数据）
    if [ "$ca_type" = "Rust" ]; then
        # 测试Rust CA的WebAuthn端点
        WEBAUTHN=$(curl -s -X POST "http://localhost:${port}/api/webauthn/register/begin" \
            -H "Content-Type: application/json" \
            -d '{"user_id":"test","user_name":"test@example.com","user_display_name":"Test","rp_name":"Test","rp_id":"localhost"}' 2>/dev/null)
    else
        # 测试Node.js CA的WebAuthn端点
        WEBAUTHN=$(curl -s -X POST "http://localhost:${port}/api/webauthn/register/begin" \
            -H "Content-Type: application/json" \
            -d '{"email":"test@example.com","displayName":"Test"}' 2>/dev/null)
    fi
    
    if echo "$WEBAUTHN" | grep -q '"challenge"'; then
        echo "  ✅ WebAuthn端点正常"
    else
        echo "  ⚠️  WebAuthn端点异常（可能需要会话）"
    fi
    
    echo "  ✅ ${ca_type} CA API测试完成"
}

# 主测试流程
main() {
    echo "开始快速连接测试..."
    echo ""
    
    # 测试Rust CA (端口3001)
    if check_ca_service "Rust" 3001; then
        quick_api_test "Rust" 3001
    else
        echo "请启动Rust CA服务："
        echo "cargo run -p airaccount-ca-extended --bin ca-server"
    fi
    
    echo ""
    
    # 测试Node.js CA (端口3002)
    if check_ca_service "Node.js" 3002; then
        quick_api_test "Node.js" 3002
    else
        echo "请启动Node.js CA服务："
        echo "cd ../../packages/airaccount-ca-nodejs && npm run dev"
    fi
    
    echo ""
    echo "📊 快速测试结果:"
    
    # 检查两个服务的状态
    RUST_OK=false
    NODEJS_OK=false
    
    if curl -s --max-time 2 http://localhost:3001/health > /dev/null; then
        RUST_OK=true
        echo "✅ Rust CA: 服务正常"
    else
        echo "❌ Rust CA: 服务未运行"
    fi
    
    if curl -s --max-time 2 http://localhost:3002/health > /dev/null; then
        NODEJS_OK=true
        echo "✅ Node.js CA: 服务正常"
    else
        echo "❌ Node.js CA: 服务未运行"
    fi
    
    echo ""
    
    if $RUST_OK && $NODEJS_OK; then
        echo "🎉 双CA服务运行正常！"
        echo ""
        echo "现在可以运行完整测试："
        echo "./run-complete-test.sh"
        echo ""
        echo "或手动测试SDK模拟器："
        echo "cd ../../packages/sdk-simulator"
        echo "npm run test-both"
    elif $RUST_OK || $NODEJS_OK; then
        echo "⚠️  部分CA服务正常，建议启动所有服务后再测试"
    else
        echo "❌ 所有CA服务都未运行"
        echo ""
        echo "启动指南："
        echo "1. 启动Rust CA: cargo run -p airaccount-ca-extended --bin ca-server"
        echo "2. 启动Node.js CA: cd ../../packages/airaccount-ca-nodejs && npm run dev"
        echo "3. 重新运行此测试"
    fi
}

main