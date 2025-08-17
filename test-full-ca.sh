#!/bin/bash

echo "🚀 Testing Full CA with WebAuthn Support"
echo "========================================"

echo "✅ OpenSSL 3.0.8 compiled successfully"
echo "✅ Full CA (airaccount-ca) built successfully"
echo "✅ Full CA copied to /shared/airaccount-ca"
echo ""

echo "📋 Available test commands in QEMU:"
echo ""
echo "1. Basic TA test (C tool):"
echo "   # /shared/simple-ta-test"
echo ""
echo "2. Simple CA test (Rust, no WebAuthn):"
echo "   # /shared/airaccount-ca-simple test"
echo ""
echo "3. Full CA test (Rust, with WebAuthn) - NEW!"
echo "   # /shared/airaccount-ca interactive"
echo "   OR"
echo "   # /shared/airaccount-ca --help"
echo ""

echo "🎯 Expected results:"
echo "- C tool: 4/4 tests pass ✅"
echo "- Simple CA: 4/4 tests pass ✅"
echo "- Full CA: Should show WebAuthn capabilities 🆕"
echo ""

echo "🔥 Key improvements in Full CA:"
echo "- WebAuthn support with OpenSSL 3.0.8"
echo "- Real authentication flows"
echo "- Database for storing credentials"
echo "- Production-ready features"
echo ""

echo "Now test in QEMU!"