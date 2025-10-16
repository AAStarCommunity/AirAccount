#!/bin/bash
# Run command in Guest VM via shared directory
# This creates a script in shared directory that Guest VM can execute

if [ -z "$1" ]; then
    echo "Usage: $0 'command'"
    echo ""
    echo "Examples:"
    echo "  $0 'ls -la /root/shared'"
    echo "  $0 'cd /root/shared && ./export_key <id> \"m/44'/60'/0'/0/0\"'"
    exit 1
fi

TIMESTAMP=$(date +%s)
SCRIPT_NAME="run_${TIMESTAMP}.sh"
OUTPUT_NAME="output_${TIMESTAMP}.txt"

# Create script in shared directory
docker exec teaclave_dev_env bash -c "cat > /opt/teaclave/shared/${SCRIPT_NAME} << 'EOFSCRIPT'
#!/bin/bash
cd /root/shared
$1 > /root/shared/${OUTPUT_NAME} 2>&1
echo \"Exit code: \$?\" >> /root/shared/${OUTPUT_NAME}
rm -f /root/shared/${SCRIPT_NAME}
EOFSCRIPT
chmod +x /opt/teaclave/shared/${SCRIPT_NAME}"

echo "📝 Script created: /root/shared/${SCRIPT_NAME}"
echo "📤 To execute in Guest VM, run:"
echo "    sh /root/shared/${SCRIPT_NAME}"
echo ""
echo "📥 Output will be in: /root/shared/${OUTPUT_NAME}"
echo ""
echo "🔍 To view output:"
echo "    docker exec teaclave_dev_env cat /opt/teaclave/shared/${OUTPUT_NAME}"
