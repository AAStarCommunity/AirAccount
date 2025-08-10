#!/bin/bash

# Licensed to AirAccount under the Apache License, Version 2.0
# Build script for AirAccount Client Application

set -e

echo "🚀 Building AirAccount Client Application"
echo "======================================="

# Set environment variables
export OPTEE_CLIENT_EXPORT="/Volumes/UltraDisk/Dev2/aastar/AirAccount/third_party/incubator-teaclave-trustzone-sdk/optee/optee_client/export_arm64"

# Check if client export exists
if [[ ! -d "$OPTEE_CLIENT_EXPORT" ]]; then
    echo "⚠️  Warning: OPTEE_CLIENT_EXPORT directory not found: $OPTEE_CLIENT_EXPORT"
    echo "    Creating mock directory for build..."
    mkdir -p "$OPTEE_CLIENT_EXPORT/usr/lib"
    mkdir -p "$OPTEE_CLIENT_EXPORT/usr/include"
    
    # Create minimal headers for build
    cat > "$OPTEE_CLIENT_EXPORT/usr/include/tee_client_api.h" << 'EOF'
#ifndef TEE_CLIENT_API_H
#define TEE_CLIENT_API_H
// Minimal TEE client API for build compatibility
#endif
EOF
fi

# Build CA
echo "Building Client Application..."
cd packages/client-ca
cargo build --release

echo "✅ Client Application built successfully!"
echo "Binary location: target/release/airaccount-ca"