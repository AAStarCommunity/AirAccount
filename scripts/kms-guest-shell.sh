#!/bin/bash
# Interactive Guest VM Shell

echo "🖥️  Connecting to Guest VM interactive shell..."
echo "    Type your commands and press Enter"
echo "    Press Ctrl+C to exit"
echo ""
echo "📝 Common commands:"
echo "    cd /root/shared"
echo "    ls -la"
echo "    ./export_key <wallet-id> \"m/44'/60'/0'/0/0\""
echo "    curl http://localhost:3000/health"
echo "    ps aux | grep kms"
echo ""
echo "Connecting..."
echo ""

# Use stdin/stdout directly for bidirectional communication
docker exec -it teaclave_dev_env bash -c "socat STDIN TCP:localhost:54320"
