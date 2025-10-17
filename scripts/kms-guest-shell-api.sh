#!/bin/bash
# Guest VM Command Execution via Shared Directory

echo "🖥️  KMS Guest VM Command Helper"
echo ""
echo "Since the Guest VM serial is occupied by API Server,"
echo "we'll use the shared directory to execute commands."
echo ""

# Check if shared directory is accessible
if docker exec teaclave_dev_env test -d /opt/teaclave/shared; then
    echo "✅ Shared directory accessible"
else
    echo "❌ Shared directory not accessible"
    exit 1
fi

echo ""
echo "📝 Available commands:"
echo "  1. List shared directory files"
echo "  2. Check API Server status"
echo "  3. List wallets via API"
echo "  4. Execute custom command"
echo "  5. Deploy new TA binary"
echo "  0. Exit"
echo ""

while true; do
    read -p "Select option (0-5): " choice
    
    case $choice in
        1)
            echo ""
            echo "📂 Files in /root/shared (Guest VM):"
            docker exec teaclave_dev_env ls -lh /opt/teaclave/shared/
            echo ""
            ;;
        2)
            echo ""
            echo "🔍 API Server Status:"
            curl -s http://localhost:3000/health | jq . || echo "API not responding"
            echo ""
            ;;
        3)
            echo ""
            echo "🔑 Listing wallets:"
            curl -s -X POST http://localhost:3000/ListKeys \
              -H "Content-Type: application/json" \
              -H "x-amz-target: TrentService.ListKeys" \
              -d '{}' | jq .
            echo ""
            ;;
        4)
            echo ""
            read -p "Enter command to execute in /opt/teaclave/shared: " cmd
            if [ -n "$cmd" ]; then
                echo "Executing: $cmd"
                docker exec teaclave_dev_env bash -c "cd /opt/teaclave/shared && $cmd"
            fi
            echo ""
            ;;
        5)
            echo ""
            echo "🚀 Deploying new TA binary..."
            echo "This will copy TA from SDK build to shared directory"
            docker exec teaclave_dev_env bash -c "
                if [ -f /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta ]; then
                    cp /root/teaclave_sdk_src/projects/web3/kms/ta/target/aarch64-unknown-optee/release/*.ta /opt/teaclave/shared/ta/
                    echo '✅ TA binary deployed'
                    ls -lh /opt/teaclave/shared/ta/*.ta
                else
                    echo '❌ TA binary not found. Need to compile first.'
                fi
            "
            echo ""
            ;;
        0)
            echo "Goodbye!"
            exit 0
            ;;
        *)
            echo "Invalid option"
            ;;
    esac
done
