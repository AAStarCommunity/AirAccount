#!/bin/bash
# Install git on MX93 (Yocto/OpenEmbedded)
# Run ON THE BOARD once after initial setup.
set -e

echo "Installing git on MX93..."

# Try opkg (Yocto package manager)
if command -v opkg &>/dev/null; then
    opkg update && opkg install git
    git --version && echo "✓ git installed via opkg"
    exit 0
fi

# Try apt (if Debian-based overlay)
if command -v apt-get &>/dev/null; then
    apt-get install -y git
    exit 0
fi

# Fall back: download static git binary
echo "No package manager found. Downloading static git binary..."
ARCH=$(uname -m)
echo "Architecture: $ARCH"
echo ""
echo "Manual option: copy a statically linked git binary to /usr/bin/git"
echo "  From your Mac: scp /usr/bin/git root@<board-ip>:/usr/bin/git"
echo ""
echo "Or set up SSH key and use scp to copy files for deploy."
exit 1
