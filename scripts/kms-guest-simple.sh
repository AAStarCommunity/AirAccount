#!/bin/bash
# Simple Guest VM Shell - sends commands via pipe

echo "🖥️  Guest VM Command Executor"
echo "================================"
echo ""

while true; do
    echo -n "guest-vm> "
    read -r cmd
    
    if [ -z "$cmd" ]; then
        continue
    fi
    
    if [ "$cmd" = "exit" ] || [ "$cmd" = "quit" ]; then
        echo "Goodbye!"
        break
    fi
    
    # Send command and capture output with timeout
    echo "$cmd" | docker exec -i teaclave_dev_env bash -c "timeout 5 socat - TCP:localhost:54320 2>&1" || echo "(no output or timeout)"
    echo ""
done
