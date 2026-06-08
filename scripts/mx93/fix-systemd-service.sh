#!/bin/bash
# Update kms-api.service with Restart=always and RestartSec=3.
# Run once after deploy to harden against unexpected crashes.
set -e

SERVICE=/etc/systemd/system/kms-api.service

echo "Current service file:"
cat "$SERVICE" || echo "(not found)"

# Write the hardened version (preserve ExecStart path from existing file)
EXEC=$(grep -m1 '^ExecStart=' "$SERVICE" 2>/dev/null || echo "ExecStart=/root/AirAccount/target/release/kms-api-server")
WORKDIR=$(grep -m1 '^WorkingDirectory=' "$SERVICE" 2>/dev/null || echo "WorkingDirectory=/root/AirAccount")
ENV_LINE=$(grep -m1 '^Environment=' "$SERVICE" 2>/dev/null || echo "")

cat > "$SERVICE" <<EOF
[Unit]
Description=AirAccount KMS API Server
After=network.target
StartLimitIntervalSec=0

[Service]
Type=simple
$WORKDIR
$EXEC
${ENV_LINE}
Restart=always
RestartSec=3
RestartPreventExitStatus=
StartLimitBurst=0

# Kill after 60s if it doesn't stop gracefully
TimeoutStopSec=60

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
echo "Service file updated:"
cat "$SERVICE"

echo ""
echo "Testing config..."
systemctl status kms-api --no-pager -n5 || true
echo "Done. Service will auto-restart on any crash or board reboot."
