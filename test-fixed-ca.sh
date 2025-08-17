#!/bin/bash

# 测试修复后的简化CA
echo "🚀 Testing Fixed Simple CA"
echo "=========================="

# 复制文件到QEMU共享目录
echo "📋 Copying fixed CA to QEMU shared directory..."
cp target/aarch64-unknown-linux-gnu/release/airaccount-ca-simple third_party/build/shared/
chmod +x third_party/build/shared/airaccount-ca-simple

echo "✅ Fixed CA copied to /shared/airaccount-ca-simple"
echo ""
echo "🎯 Now run in QEMU terminal:"
echo "   # /shared/airaccount-ca-simple test"
echo ""
echo "Expected result: All 4 tests should pass (100%)"
echo "- Test 1: Hello World ✅"
echo "- Test 2: Echo ✅" 
echo "- Test 3: Version ✅"
echo "- Test 4: Create Wallet ✅ (should now pass!)"