# AirAccount SDK Test Suite Documentation

Complete testing infrastructure for AirAccount SDK with dual CA support and blockchain integration.

## 🎯 Overview

This test suite provides comprehensive testing for the AirAccount ecosystem, covering the complete user lifecycle from account creation to blockchain transactions. It supports testing with both Rust and Node.js Client Applications (CAs) and includes real blockchain integration via Anvil.

## 🏗️ Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Test Scripts              │    │  AirAccount SDK            │    │   CA Layer      │
│                             │    │                            │    │                 │
│ • Lifecycle                 │◄──►│ • WebAuthn              │◄──►│ • Rust CA:3001  │
│ • Multi-user                │◄──►│ • Wallet Mgmt           │◄──►│ • Node.js:3002  │
│ • Performance               │◄──►│ • Transfers             │◄──►│ • Session Mgmt  │
│                             │    │                            │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                        │
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│     Anvil                   │    │   QEMU TEE                 │    │   TA (Rust)     │
│                             │    │                            │    │                 │
│ • Local testnet             │◄──►│ • OP-TEE 4.7             │◄──►│ • Private keys  │
│ • Balance query             │    │ • ARM TrustZone            │    │ • TX signing    │
│ • TX broadcast              │    │ • Hardware sim             │    │ • Hybrid entropy│
│                             │    │                            │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## 📋 Test Components

### 1. Core Test Files

#### `anvil-lifecycle-test.js`
**Purpose**: Complete user lifecycle testing with Anvil blockchain integration
- Account creation with TEE security
- Real blockchain funding and balance queries
- Transfer execution with TEE signing
- Multi-account interaction testing

#### `demo-blockchain-integration.js`
**Purpose**: Advanced multi-user demo with performance analysis
- Comprehensive user registration flows
- Performance comparison between CA types
- Detailed reporting and statistics
- Real-time blockchain interaction

#### `test-ca-integration.js`
**Purpose**: Basic CA integration testing
- SDK initialization and health checks
- WebAuthn registration simulation
- Wallet operations testing
- Both Rust and Node.js CA support

#### `demo-full-flow.js`
**Purpose**: Complete flow demonstration
- User registration and login simulation
- Wallet balance and transfer operations
- Recovery information management
- Multi-user scenario testing

### 2. SDK Components

#### `AirAccountSDKSimulator`
Enhanced SDK for testing with dual CA support:
```javascript
const sdk = new AirAccountSDKSimulator({ ca: 'nodejs' }); // or 'rust'
await sdk.initialize();
await sdk.registerWithWebAuthn(userInfo);
const account = await sdk.createAccount(userInfo, passkeyData);
const balance = await sdk.getBalance(walletId);
const result = await sdk.transfer(walletId, toAddress, amount);
```

#### `BlockchainIntegratedSDK`
Extended SDK with blockchain verification:
```javascript
const sdk = new BlockchainIntegratedSDK({ ca: 'nodejs' });
await sdk.setBlockchainProvider('http://127.0.0.1:8545');
const balance = await sdk.getBalance(walletId); // Includes blockchain verification
```

## 🚀 Running Tests

### Prerequisites

1. **Install Foundry** (includes Anvil):
```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

2. **Install Dependencies**:
```bash
cd packages/airaccount-sdk-test
npm install
```

3. **Setup libteec Library** (for Rust CA support):
```bash
# Create mock libteec library for macOS development
mkdir -p /tmp/mock_tee/usr/lib
cd /tmp/mock_tee/usr/lib

# Create mock libteec.c
cat > libteec.c << 'EOF'
// Mock libteec implementation for macOS development
#include <stdint.h>
#include <stdio.h>

// Mock TEEC functions that satisfy linking requirements
int TEEC_InitializeContext(void) { return 0; }
void TEEC_FinalizeContext(void) {}
int TEEC_OpenSession(void) { return 0; }
void TEEC_CloseSession(void) {}
int TEEC_InvokeCommand(void) { return 0; }
int TEEC_AllocateSharedMemory(void) { return 0; }
void TEEC_ReleaseSharedMemory(void) {}
EOF

# Compile to dynamic library
gcc -shared -o libteec.dylib libteec.c

# Set environment variable for linking
export DYLD_LIBRARY_PATH="/tmp/mock_tee/usr/lib:$DYLD_LIBRARY_PATH"
```

4. **Start Required Services**:
- **Node.js CA**: `http://localhost:3002` (required)
- **Rust CA**: `http://localhost:3001` (optional, requires libteec setup)
- **QEMU OP-TEE**: TEE environment with AirAccount TA

### Test Execution Commands

#### Basic Integration Tests
```bash
# Test both CA types
npm run test-both

# Test specific CA
npm run test-nodejs
npm run test-rust

# Basic integration test
npm test
```

#### Lifecycle and Blockchain Tests
```bash
# Complete lifecycle with Anvil
npm run lifecycle

# Advanced blockchain demo
npm run demo-blockchain

# Basic Anvil testing
npm run test-anvil
```

#### Demo Applications
```bash
# Original demo flow
npm run demo

# Blockchain integration demo
npm run blockchain
```

## 🧪 Test Scenarios

### Scenario 1: Basic Lifecycle Test

**Flow**: Single user account creation → funding → balance query → transfer

```bash
npm run lifecycle
```

**Expected Results**:
- ✅ Account creation with TEE key generation
- ✅ Anvil blockchain funding (5.0 ETH)
- ✅ Balance verification via TEE and blockchain
- ✅ Transfer execution with TEE signing
- ✅ Final balance confirmation

### Scenario 2: Multi-User Blockchain Demo

**Flow**: Multiple users → cross-transfers → performance analysis

```bash
npm run demo-blockchain
```

**Test Users**:
- **Alice Johnson**: Node.js CA, 10.0 ETH initial funding
- **Bob Smith**: Rust CA, 8.0 ETH initial funding
- **Charlie Brown**: Node.js CA, 5.0 ETH initial funding

**Operations**:
- Alice → Bob: 2.5 ETH
- Bob → Charlie: 1.0 ETH
- Charlie → Alice: 0.5 ETH

### Scenario 3: CA Performance Comparison

**Metrics Tested**:
- Balance query response time
- Wallet list operation time
- Account creation duration
- Error rate and reliability

### Scenario 4: WebAuthn Flow Testing

**Components**:
- Registration challenge generation
- Passkey credential simulation
- Authentication flow validation
- Session management verification

## 📊 Expected Performance Metrics

### Response Times (Typical Values)
- **Account Creation**: 500-1500ms (includes TEE key generation)
- **Balance Query**: 50-200ms (TEE + blockchain verification)
- **Transfer Signing**: 100-500ms (TEE signature generation)
- **Blockchain Confirmation**: 2000ms (Anvil block time)

### CA Comparison
- **Rust CA**:
  - Lower latency (direct TEE communication)
  - Minimal overhead
  - Native performance

- **Node.js CA**:
  - Slightly higher latency (HTTP proxy)
  - Rich API features
  - Session management

## 🔍 Test Output Examples

### Successful Lifecycle Test
```
🧪 [12:34:56] Starting Anvil blockchain...
✅ [12:34:57] Anvil blockchain started successfully
⛓️ [12:34:58] Connected to network: anvil (chainId: 31337)
👤 [12:35:00] Account created successfully:
👤 [12:35:00]   - Wallet ID: 1
👤 [12:35:00]   - Address: 0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A
⛓️ [12:35:02] Funding transaction sent: 0x1234567890abcdef...
✅ [12:35:03] Funding confirmed in block 1
✅ [12:35:03] Account balance: 5.0 ETH
💸 [12:35:05] Transfer: 1.0 ETH from 0x742d... to 0x8ba1...
✅ [12:35:07] Transfer successful - Transaction: 0xabcdef...
```

### Multi-User Demo Report
```
🎭 AIRACCOUNT BLOCKCHAIN DEMO - FINAL REPORT
================================================================================

📊 DEMO STATISTICS:
  ├─ Total Users Created: 3
  ├─ CA Types Tested: NODEJS, RUST
  └─ Total Transactions: 12

👥 USER PROFILES:
  User 1: Alice Johnson
    ├─ Email: alice@airaccount.dev
    ├─ CA Type: NODEJS
    ├─ Wallet ID: 1
    ├─ Address: 0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A
    └─ Transactions: 4

🔗 ARCHITECTURE VERIFICATION:
  ├─ ✅ Demo Layer: User interaction simulation
  ├─ ✅ SDK Layer: TypeScript/JavaScript interface
  ├─ ✅ CA Layer: Rust & Node.js client applications
  ├─ ✅ TA Layer: Trusted application in TEE
  ├─ ✅ TEE Layer: QEMU OP-TEE hardware simulation
  └─ ✅ Blockchain: Anvil local testnet integration

🛡️ SECURITY FEATURES DEMONSTRATED:
  ├─ ✅ WebAuthn Registration: Passkey-based authentication
  ├─ ✅ TEE Key Storage: Private keys never leave secure hardware
  ├─ ✅ Hybrid Entropy: Secure key generation in TEE
  ├─ ✅ Transaction Signing: Hardware-based signature generation
  └─ ✅ Client Control: User manages own recovery credentials

🎯 TEST SCENARIOS COMPLETED:
  ├─ ✅ Multi-user registration with different CAs
  ├─ ✅ Account funding and balance verification
  ├─ ✅ Cross-user transfers with TEE signing
  ├─ ✅ Real blockchain interaction via Anvil
  ├─ ✅ Performance comparison between CA types
  └─ ✅ End-to-end workflow validation
```

## 🔧 API Testing Examples

### Account Creation
```javascript
// Create SDK instance
const sdk = new AirAccountSDKSimulator({ ca: 'nodejs' });
await sdk.initialize();

// User registration
const userInfo = {
  email: 'test@airaccount.dev',
  displayName: 'Test User'
};

await sdk.registerWithWebAuthn(userInfo);

// Passkey data simulation
const passkeyData = {
  credentialId: 'test_credential_123',
  publicKeyBase64: Buffer.from('test_public_key').toString('base64')
};

// Account creation in TEE
const account = await sdk.createAccount(userInfo, passkeyData);
console.log('Wallet ID:', account.walletId);
console.log('Address:', account.address);
```

### Balance Query
```javascript
// Query balance through TEE
const balance = await sdk.getBalance(walletId);

// With blockchain verification
const enhancedSDK = new BlockchainIntegratedSDK({ ca: 'nodejs' });
await enhancedSDK.setBlockchainProvider('http://127.0.0.1:8545');
const verifiedBalance = await enhancedSDK.getBalance(walletId);

console.log('TEE Balance:', balance);
console.log('Blockchain Balance:', verifiedBalance.blockchain_balance_eth);
```

### Transfer Execution
```javascript
// Execute transfer with TEE signing
const transferResult = await sdk.transfer(
  fromWalletId,
  '0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A',
  '1.5' // ETH amount
);

console.log('Transaction Hash:', transferResult.transaction_hash);
console.log('Signature:', transferResult.signature);
```

## 🛠️ Troubleshooting

### Common Issues

#### 1. Anvil Startup Failures
```bash
# Check Foundry installation
anvil --version

# Reinstall if needed
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

#### 2. CA Connection Issues
```bash
# Verify Node.js CA
curl http://localhost:3002/health

# Verify Rust CA
curl http://localhost:3001/health

# Expected response
{"status":"healthy","services":{"tee":{"connected":true}}}
```

#### 3. TEE Connection Problems
- Ensure QEMU OP-TEE is running
- Verify AirAccount TA is loaded: `/lib/optee_armtz/11223344-5566-7788-99aa-bbccddeeff01.ta`
- Check TEE devices: `ls -la /dev/tee*`

#### 4. SDK Import Errors
```bash
# Install dependencies
npm install

# Check Node.js version
node --version  # Should be ≥16.0.0

# Verify package.json type
grep '"type": "module"' package.json
```

### Debug Mode
Enable detailed logging:
```bash
DEBUG=1 npm run demo-blockchain
```

### Test Isolation
Run tests in isolation:
```bash
# Clean start
pkill -f anvil
npm run demo-blockchain
```

## 🔒 Security Verification

### Verified Security Features
- **Private Key Isolation**: Keys generated and stored only in TEE
- **WebAuthn Authentication**: Passkey-based user authentication
- **Hybrid Entropy**: Secure random generation combining multiple sources
- **Hardware Signing**: All transaction signatures generated in TEE
- **Client Control**: Users maintain their own recovery credentials
- **Session Security**: Secure session management across CA types

### Security Test Cases
- **Key Extraction**: Verify private keys cannot be extracted from TEE
- **Signature Verification**: Confirm all signatures are TEE-generated
- **Authentication**: Test WebAuthn challenge/response flows
- **Session Management**: Verify secure session creation/validation
- **Recovery**: Test user-controlled credential recovery scenarios

## 📈 Performance Analysis

### Benchmarking Results
The test suite includes performance comparison between CA types:

```javascript
// Example performance test results
{
  "nodejs": {
    "balanceQueryTime": "156.42ms",
    "walletListTime": "89.33ms",
    "totalTestTime": "245.75ms",
    "status": "success"
  },
  "rust": {
    "balanceQueryTime": "87.21ms",
    "walletListTime": "45.67ms",
    "totalTestTime": "132.88ms",
    "status": "success"
  }
}
```

### Optimization Opportunities
- **Rust CA**: Optimal for performance-critical applications
- **Node.js CA**: Better for rapid development and rich API features
- **Hybrid Approach**: Use different CAs for different use cases

## 🤝 Extending the Test Suite

### Adding New Test Scenarios

1. **Create Test Function**:
```javascript
async testNewScenario() {
  this.log('Testing new scenario...');
  // Test implementation
}
```

2. **Add to Demo Flow**:
```javascript
await this.testNewScenario();
```

3. **Update Package Scripts**:
```json
{
  "scripts": {
    "test-new": "node new-test-scenario.js"
  }
}
```

### Contributing Guidelines
- Follow existing logging patterns
- Include error handling and cleanup
- Add comprehensive documentation
- Test with both CA types
- Verify blockchain integration

## 📚 Related Documentation

- [Main Testing Guide](../../TESTING_GUIDE.md)
- [SDK Documentation](../node-sdk/README.md)
- [Node.js CA API](../airaccount-ca-nodejs/README.md)
- [WebAuthn Integration Guide](../airaccount-ca-nodejs/src/routes/webauthn.ts)
- [Blockchain Demo README](./README-BLOCKCHAIN-DEMO.md)

This comprehensive test suite ensures the AirAccount ecosystem functions correctly across all components, providing confidence in the security, performance, and reliability of the complete system.
