#!/bin/bash

# Licensed to AirAccount under the Apache License, Version 2.0
# Build script for AirAccount TEE components

set -e

echo "🏗️  Building AirAccount TEE Components"
echo "=================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the correct directory
if [[ ! -f "Cargo.toml" ]]; then
    print_error "Please run this script from the AirAccount root directory"
    exit 1
fi

# Step 1: Build protocol definitions
print_status "Building protocol definitions..."
cargo build -p airaccount-proto
print_success "Protocol definitions built"

# Step 2: Build core logic
print_status "Building core logic..."
cargo build -p airaccount-core-logic
print_success "Core logic built"

# Step 3: Build CA (Client Application)
print_status "Building Client Application..."
cargo build -p airaccount-ca --release
print_success "Client Application built"

# Step 4: Try to build TA (this might fail without proper environment)
print_status "Attempting to build Trusted Application..."
cd packages/ta-arm-trustzone

if command -v cargo &> /dev/null; then
    # Check if we have the required environment
    if [[ -d "../../third_party/incubator-teaclave-trustzone-sdk/optee" ]]; then
        print_warning "Building TA requires OP-TEE environment setup"
        print_status "Setting up environment..."
        
        export STD=y
        if [[ -f "../../third_party/incubator-teaclave-trustzone-sdk/environment" ]]; then
            source ../../third_party/incubator-teaclave-trustzone-sdk/environment
            print_status "Building TA with OP-TEE environment..."
            # cargo build --target aarch64-unknown-optee --release || print_warning "TA build failed - OP-TEE environment may not be fully set up"
            print_warning "Skipping TA build - requires full OP-TEE setup"
        else
            print_warning "OP-TEE environment file not found - skipping TA build"
        fi
    else
        print_warning "OP-TEE libraries not found - skipping TA build"
    fi
else
    print_error "Cargo not found"
    exit 1
fi

cd ../..

# Step 5: Run tests
print_status "Running tests..."
cargo test -p airaccount-proto --lib
cargo test -p airaccount-core-logic --lib
# Note: CA tests require TA to be deployed, so we skip them for now

print_success "Build process completed!"

# Summary
echo ""
echo "📋 Build Summary:"
echo "=================="
echo "✅ Protocol definitions: Built"
echo "✅ Core logic: Built"  
echo "✅ Client Application: Built"
echo "⚠️  Trusted Application: Requires OP-TEE environment"
echo ""
echo "📖 Next Steps:"
echo "- Deploy TA to TEE environment for testing"
echo "- Run integration tests with: ./packages/client-ca/target/release/airaccount-ca test"
echo ""
print_success "AirAccount TEE components are ready!"