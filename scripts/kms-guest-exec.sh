#!/bin/bash
# Execute command in Guest VM (non-interactive)
# Usage: ./scripts/kms-guest-exec.sh "command"

if [ -z "$1" ]; then
    echo "Usage: $0 'command'"
    echo "Example: $0 'cd /root/shared && ls -la'"
    exit 1
fi

COMMAND="$1"

echo "📤 Sending command to Guest VM: $COMMAND"
echo ""

docker exec teaclave_dev_env bash -c "echo '$COMMAND' | socat - TCP:localhost:54320" 2>&1 | grep -v "socat"
