# AirAccount Blockchain Integration Demo

Complete user lifecycle testing with real blockchain integration using Anvil and dual CA support.

## 🎯 Demo Overview

This demo demonstrates the complete AirAccount user lifecycle:

1. **User Registration**: WebAuthn-based registration with passkey creation
2. **Account Creation**: TEE-based wallet creation with private key generation
3. **Funding**: Receive test ETH from Anvil faucet
4. **Balance Queries**: Query balances through both TEE and blockchain
5. **Transfers**: Execute transfers with TEE-based transaction signing
6. **Multi-CA Testing**: Compare Rust CA and Node.js CA performance

## 🏗️ Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Demo Script   │    │      SDK        │    │   CA (Rust/JS)  │
│                 │    │                 │    │                 │
│ • User flows    │◄──►│ • API calls     │◄──►│ • WebAuthn API  │
│ • Test scenarios│    │ • Auth mgmt     │    │ • Wallet API    │
│                 │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                        │
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│     Anvil       │    │   QEMU TEE      │    │   TA (Rust)     │
│                 │    │                 │    │                 │
│ • Local chain   │◄──►│ • OP-TEE 4.7    │◄──►│ • Private keys  │
│ • Balance query │    │ • ARM TrustZone │    │ • Signing       │
│ • Tx broadcast  │    │ • Hardware sim  │    │ • Hybrid entropy│
│                 │    │                 │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 📋 Prerequisites

### Required Software
```bash
# Foundry (includes Anvil)
curl -L https://foundry.paradigm.xyz | bash
foundryup

# Node.js dependencies
npm install
```

### Required Services
- **Node.js CA**: Running on `http://localhost:3002`
- **Rust CA**: Running on `http://localhost:3001` (optional)
- **QEMU OP-TEE**: TEE environment with AirAccount TA loaded

## 🚀 Running the Demos

### 1. Basic Lifecycle Test
```bash
npm run lifecycle
```
Tests complete account lifecycle with Anvil blockchain integration.

### 2. Comprehensive Blockchain Demo
```bash
npm run demo-blockchain
```
Advanced demo with multi-user scenarios, performance testing, and detailed reporting.

### 3. Simple Anvil Test
```bash
npm run test-anvil
```
Basic lifecycle test focusing on blockchain interaction.

### 4. CA Integration Tests
```bash
# Test both CAs
npm run test-both

# Test specific CA
npm run test-nodejs
npm run test-rust
```

## 🧪 Test Scenarios

### Scenario 1: Single User Lifecycle
1. **Registration**: Create user with WebAuthn
2. **Wallet Creation**: Generate wallet in TEE
3. **Funding**: Receive 5 ETH from faucet
4. **Balance Check**: Verify balance through TEE and blockchain
5. **Transfer**: Send 1 ETH to another address
6. **Final Verification**: Confirm final balance

### Scenario 2: Multi-User Interaction
1. **Create Users**: Alice (Node.js CA), Bob (Rust CA), Charlie (Node.js CA)
2. **Fund Accounts**: Alice: 10 ETH, Bob: 8 ETH, Charlie: 5 ETH
3. **Cross-user Transfers**: 
   - Alice → Bob: 2.5 ETH
   - Bob → Charlie: 1.0 ETH
   - Charlie → Alice: 0.5 ETH
4. **Performance Testing**: Compare CA response times
5. **Final Report**: Complete statistics and verification

### Scenario 3: CA Performance Comparison
- **Rust CA**: Native performance, direct TEE communication
- **Node.js CA**: HTTP API, session management, proxy to TEE
- **Metrics**: Response times, error rates, feature completeness

## 📊 Expected Output

### Successful Demo Output
```
🎭 AIRACCOUNT COMPREHENSIVE BLOCKCHAIN DEMO
================================================================================
Demonstration: Complete user lifecycle with dual CA support
Architecture: Demo → SDK → CA → TA → TEE → Anvil Blockchain

🚀 Phase 1: Environment Setup
--------------------------------------------------
⛓️ [12:34:56] [ANVIL] Anvil blockchain ready for connections
✅ [12:34:57] [BLOCKCHAIN] Connected to anvil (Chain ID: 31337)
⛓️ [12:34:57] [BLOCKCHAIN] Current block: 0
⛓️ [12:34:57] [BLOCKCHAIN] Funder balance: 10000.0 ETH

👥 Phase 2: Multi-User Registration
--------------------------------------------------
✅ [12:34:58] [USER] User Alice Johnson profile created with NODEJS CA
✅ [12:34:59] [USER] Registration complete for Alice Johnson:
👤 [12:34:59] [USER]   ├─ Wallet ID: 1
👤 [12:34:59] [USER]   ├─ Address: 0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A
👤 [12:34:59] [USER]   └─ CA Type: NODEJS

💰 Phase 3: Account Funding
--------------------------------------------------
⛓️ [12:35:01] [FUNDING] Funding tx sent: 0x1234567890abcdef...
✅ [12:35:02] [FUNDING] Funding confirmed in block 1
✅ [12:35:02] [FUNDING] Alice Johnson's balance: 10.0 ETH

🔍 Phase 4: Balance Query Testing
--------------------------------------------------
✅ [12:35:03] [BALANCE] Balance query results for Alice Johnson:
📱 [12:35:03] [BALANCE]   ├─ TEE Query: {"success":true,"wallet":{"balance":{"eth":"10.0"}}}...
⛓️ [12:35:03] [BALANCE]   └─ Blockchain: 10.0 ETH

💸 Phase 5: Transfer Execution
--------------------------------------------------
💸 [12:35:05] [TRANSFER] Transfer: 2.5 ETH from Alice Johnson to Bob Smith
✅ [12:35:06] [TRANSFER] TEE transfer result: {"transaction_hash":"0xabcdef..."}
⛓️ [12:35:07] [TRANSFER] Blockchain transaction: 0xfedcba...

✅ Phase 7: Final State Verification
--------------------------------------------------
✅ [12:35:10] [VERIFICATION] Alice Johnson final balance: 7.5 ETH
✅ [12:35:10] [VERIFICATION] Bob Smith final balance: 9.5 ETH

🎉 COMPREHENSIVE DEMO COMPLETED SUCCESSFULLY!
```

## 🔍 Troubleshooting

### Common Issues

1. **Anvil Not Starting**
   ```bash
   # Check if foundry is installed
   anvil --version
   
   # If not installed
   curl -L https://foundry.paradigm.xyz | bash
   foundryup
   ```

2. **CA Services Not Running**
   ```bash
   # Check Node.js CA
   curl http://localhost:3002/health
   
   # Check Rust CA
   curl http://localhost:3001/health
   ```

3. **TEE Connection Issues**
   - Ensure QEMU OP-TEE is running
   - Verify TA is loaded in TEE environment
   - Check `/dev/tee*` devices are available

4. **SDK Import Errors**
   ```bash
   # Ensure dependencies are installed
   npm install
   
   # Check Node.js version
   node --version  # Should be ≥16.0.0
   ```

### Debug Mode
Run with detailed logging:
```bash
DEBUG=1 npm run demo-blockchain
```

## 📈 Performance Metrics

### Expected Performance (Typical Values)
- **Account Creation**: 500-1500ms (includes TEE key generation)
- **Balance Query**: 50-200ms (TEE + blockchain verification)
- **Transfer Signing**: 100-500ms (TEE signature generation)
- **Blockchain Confirmation**: 2000ms (Anvil block time)

### CA Comparison
- **Rust CA**: Lower latency, direct TEE communication
- **Node.js CA**: Slightly higher latency, rich API features

## 🛡️ Security Features Verified

- ✅ **Private Key Security**: Keys never leave TEE hardware
- ✅ **WebAuthn Authentication**: Passkey-based user authentication
- ✅ **Hybrid Entropy**: Secure random generation in TEE
- ✅ **Transaction Signing**: Hardware-based signature generation
- ✅ **Client Control**: User manages own recovery credentials
- ✅ **Multi-CA Support**: Flexible client application choice

## 📚 Related Documentation

- [Main Testing Guide](../../TESTING_GUIDE.md)
- [SDK Documentation](../node-sdk/README.md)
- [CA API Specifications](../airaccount-ca-nodejs/README.md)
- [WebAuthn Integration](../airaccount-ca-nodejs/src/routes/webauthn.ts)

## 🤝 Contributing

To extend the demo with additional scenarios:

1. Add new test functions to the demo classes
2. Update the demo execution flow
3. Add corresponding npm scripts
4. Update this documentation

The demo framework is designed to be extensible and can easily accommodate new testing scenarios.