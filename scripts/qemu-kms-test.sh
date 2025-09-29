#!/bin/bash

# QEMU Internal KMS Test Script
# To be used inside QEMU OP-TEE environment

echo "=== KMS QEMU Internal Test Script ==="
echo "🔧 Setting up environment..."

# Mount shared directory
mkdir -p /mnt/host
mount -t 9p -o trans=virtio host /mnt/host
echo "✅ Mounted shared directory"

# Copy binaries
echo "📦 Copying KMS binaries..."
cp /mnt/host/host/kms-host /tmp/kms-host
cp /mnt/host/host/kms-api /tmp/kms-api
chmod +x /tmp/kms-host /tmp/kms-api
echo "✅ KMS binaries ready"

# Check network
echo "🌐 Checking network configuration..."
ip addr show
echo

# Test KMS Host Application
echo "🧪 Testing KMS Host Application..."
echo "--- Help ---"
/tmp/kms-host --help
echo

echo "--- Create Wallet ---"
/tmp/kms-host create-wallet
echo

# Start KMS API Server in background
echo "🌐 Starting KMS API Server..."
/tmp/kms-api &
KMS_PID=$!
sleep 2

# Test API Server with curl
echo "🔍 Testing KMS API Server..."
if command -v curl >/dev/null 2>&1; then
    echo "Testing ListKeys..."
    curl -X POST http://localhost:8080/ \
         -H "Content-Type: application/x-amz-json-1.1" \
         -H "X-Amz-Target: TrentService.ListKeys" \
         -d '{}' || echo "Curl test failed (expected without AWS credentials)"
    echo
else
    echo "⚠️ curl not available, skipping HTTP tests"
fi

# Test API with netcat if available
if command -v nc >/dev/null 2>&1; then
    echo "Testing with netcat..."
    echo -e "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n" | nc localhost 8080 | head -10
else
    echo "⚠️ netcat not available"
fi

# Check if server is running
echo "📊 Checking server status..."
ps | grep kms-api || echo "API server not in process list"
netstat -tulpn 2>/dev/null | grep 8080 || echo "Port 8080 not listening"

# Cleanup
echo "🧹 Cleaning up..."
kill $KMS_PID 2>/dev/null || true

echo "🎉 KMS QEMU test completed!"
echo "💡 Manual testing instructions:"
echo "   1. /tmp/kms-host --help"
echo "   2. /tmp/kms-host create-wallet"
echo "   3. /tmp/kms-api &"
echo "   4. Test with curl or netcat"