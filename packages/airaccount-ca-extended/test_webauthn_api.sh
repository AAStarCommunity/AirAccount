#!/bin/bash

# WebAuthn API 测试脚本
# 测试 airaccount-ca-extended 的简化 WebAuthn API

set -e

API_BASE="http://localhost:3001"

echo "🧪 Testing AirAccount CA Extended WebAuthn API"
echo "==============================================="

# 测试健康检查
echo "1. 🩺 Testing Health Check..."
curl -s "$API_BASE/health" | jq '.'
echo ""

# 测试WebAuthn注册开始
echo "2. 🔐 Testing WebAuthn Registration Begin..."
curl -s -X POST "$API_BASE/api/webauthn/register/begin" \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "user123",
    "user_name": "test@example.com",
    "user_display_name": "Test User",
    "rp_name": "AirAccount Test",
    "rp_id": "localhost"
  }' | jq '.'
echo ""

# 测试WebAuthn认证开始
echo "3. 🔓 Testing WebAuthn Authentication Begin..."
curl -s -X POST "$API_BASE/api/webauthn/authenticate/begin" \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "user123"
  }' | jq '.'
echo ""

# 测试WebAuthn认证完成（模拟）
echo "4. 🔍 Testing WebAuthn Authentication Finish..."
curl -s -X POST "$API_BASE/api/webauthn/authenticate/finish" \
  -H "Content-Type: application/json" \
  -d '{
    "credential_id": "mock_credential_123",
    "client_data_json": "eyJ0eXBlIjoid2ViYXV0aG4uZ2V0IiwiY2hhbGxlbmdlIjoidGVzdCIsIm9yaWdpbiI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMSJ9",
    "authenticator_data": "mock_auth_data",
    "signature": "mock_signature",
    "challenge": "test_challenge"
  }' | jq '.'
echo ""

echo "✅ WebAuthn API tests completed!"
echo ""
echo "📝 Notes:"
echo "- 这是简化的WebAuthn实现，仅提供challenge-response机制"
echo "- 真实的WebAuthn验证需要在浏览器中使用Simple WebAuthn npm包"
echo "- CA后端只负责challenge生成和基本验证"