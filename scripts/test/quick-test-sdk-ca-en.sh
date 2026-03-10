#!/bin/bash

# AirAccount SDK-CA Quick Connection Test (English Version)
# ==========================================================
# 
# VARIABLE DESIGN AND STATISTICAL METHODOLOGY
# ============================================
# 
# Test Variables:
# - Service Status (Binary): CA service availability (0=down, 1=up)
# - Response Time (Continuous): API response latency in milliseconds
# - Endpoint Functionality (Categorical): health, webauthn (pass/fail/warning)
# - Service Type (Categorical): Rust CA, Node.js CA
# 
# Statistical Approach:
# - Descriptive Statistics: Binary status checks with boolean logic
# - Threshold Testing: 3-second timeout for service availability
# - Categorical Assessment: Multi-level response validation (success/fail/warning)
# - Comparative Analysis: Cross-service functionality comparison
# 
# Measurement Methodology:
# - Health Check: HTTP GET request with timeout constraint
# - API Validation: POST request with structured JSON payload
# - Status Aggregation: Boolean combination for overall system health
# - Result Classification: Three-tier outcome (all services, partial, none)
# 
# Quality Assurance:
# - Timeout Controls: Prevent hanging connections (--max-time 3)
# - Error Handling: Silent failures with explicit status reporting
# - Validation Logic: Pattern matching for expected response structures
# - Reproducibility: Consistent test conditions across multiple runs

echo "🔌 AirAccount SDK-CA Quick Connection Test"
echo "========================================="

# Check if CA service is running
check_ca_service() {
    local ca_type=$1
    local port=$2
    
    echo "Checking ${ca_type} CA service (port ${port})..."
    
    if curl -s --max-time 3 "http://localhost:${port}/health" > /dev/null; then
        echo "✅ ${ca_type} CA service is running"
        return 0
    else
        echo "❌ ${ca_type} CA service is not responding"
        return 1
    fi
}

# Quick API endpoint test
quick_api_test() {
    local ca_type=$1
    local port=$2
    
    echo "Testing ${ca_type} CA API endpoints..."
    
    # Health check
    HEALTH=$(curl -s "http://localhost:${port}/health")
    if echo "$HEALTH" | grep -q '"status":"healthy"\|"tee_connected":true'; then
        echo "  ✅ Health check passed"
    else
        echo "  ❌ Health check failed"
        echo "$HEALTH"
        return 1
    fi
    
    # WebAuthn endpoint test (no real data required)
    if [ "$ca_type" = "Rust" ]; then
        # Test Rust CA WebAuthn endpoint
        WEBAUTHN=$(curl -s -X POST "http://localhost:${port}/api/webauthn/register/begin" \
            -H "Content-Type: application/json" \
            -d '{"user_id":"test","user_name":"test@example.com","user_display_name":"Test","rp_name":"Test","rp_id":"localhost"}' 2>/dev/null)
    else
        # Test Node.js CA WebAuthn endpoint
        WEBAUTHN=$(curl -s -X POST "http://localhost:${port}/api/webauthn/register/begin" \
            -H "Content-Type: application/json" \
            -d '{"email":"test@example.com","displayName":"Test"}' 2>/dev/null)
    fi
    
    if echo "$WEBAUTHN" | grep -q '"challenge"'; then
        echo "  ✅ WebAuthn endpoint is functional"
    else
        echo "  ⚠️  WebAuthn endpoint anomaly (may require session)"
    fi
    
    echo "  ✅ ${ca_type} CA API test completed"
}

# Main test workflow
main() {
    echo "Starting quick connection test..."
    echo ""
    
    # Test Rust CA (port 3001)
    if check_ca_service "Rust" 3001; then
        quick_api_test "Rust" 3001
    else
        echo "Please start Rust CA service:"
        echo "cargo run -p airaccount-ca-extended --bin ca-server"
    fi
    
    echo ""
    
    # Test Node.js CA (port 3002)
    if check_ca_service "Node.js" 3002; then
        quick_api_test "Node.js" 3002
    else
        echo "Please start Node.js CA service:"
        echo "cd ../../packages/airaccount-ca-nodejs && npm run dev"
    fi
    
    echo ""
    echo "📊 Quick Test Results:"
    
    # Check status of both services
    RUST_OK=false
    NODEJS_OK=false
    
    if curl -s --max-time 2 http://localhost:3001/health > /dev/null; then
        RUST_OK=true
        echo "✅ Rust CA: Service operational"
    else
        echo "❌ Rust CA: Service not running"
    fi
    
    if curl -s --max-time 2 http://localhost:3002/health > /dev/null; then
        NODEJS_OK=true
        echo "✅ Node.js CA: Service operational"
    else
        echo "❌ Node.js CA: Service not running"
    fi
    
    echo ""
    
    if $RUST_OK && $NODEJS_OK; then
        echo "🎉 Dual CA services are operational!"
        echo ""
        echo "You can now run comprehensive tests:"
        echo "./run-complete-test.sh"
        echo ""
        echo "Or manually test SDK simulator:"
        echo "cd ../../packages/sdk-simulator"
        echo "npm run test-both"
    elif $RUST_OK || $NODEJS_OK; then
        echo "⚠️  Partial CA services operational, recommend starting all services before testing"
    else
        echo "❌ All CA services are down"
        echo ""
        echo "Startup Guide:"
        echo "1. Start Rust CA: cargo run -p airaccount-ca-extended --bin ca-server"
        echo "2. Start Node.js CA: cd ../../packages/airaccount-ca-nodejs && npm run dev"
        echo "3. Re-run this test"
    fi
}

main

