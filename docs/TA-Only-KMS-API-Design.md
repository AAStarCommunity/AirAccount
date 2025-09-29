# TA-Only KMS API Design

**Created:** Mon Sep 29 11:52:22 +07 2025
**Status:** Corrected Architecture
**Version:** v2.1.0-ta-only

## Architecture Principle

**CRITICAL**: All APIs must be implemented **ONLY** using eth_wallet Trusted Application (TA) capabilities. No independent Rust crypto implementations allowed.

## eth_wallet TA Real Capabilities

Based on actual TA source code analysis:

### 🔒 TA Commands (4 total):
1. **CreateWallet** - Generate HD wallet with mnemonic in TEE
2. **RemoveWallet** - Securely delete wallet from TEE storage
3. **DeriveAddress** - BIP32 HD address derivation in TEE
4. **SignTransaction** - EIP-155 transaction signing in TEE

### 🔒 TA Input/Output Types:
```rust
// CreateWallet
Input: {}  // No parameters needed
Output: { wallet_id: Uuid, mnemonic: String }

// RemoveWallet
Input: { wallet_id: Uuid }
Output: {}

// DeriveAddress
Input: { wallet_id: Uuid, hd_path: String }
Output: { address: [u8; 20], public_key: Vec<u8> }

// SignTransaction
Input: { wallet_id: Uuid, hd_path: String, transaction: EthTransaction }
Output: { signature: Vec<u8> }
```

## KMS API Design (TA-Only)

### API Mapping Strategy:
Map AWS KMS-compatible endpoints to eth_wallet TA commands:

#### 1. TrentService.CreateAccount → TA CreateWallet
**Purpose**: Create HD account using TA wallet generation
**Request**:
```json
{
  "Description": "Account description (optional)"
}
```
**Response**:
```json
{
  "AccountMetadata": {
    "AccountId": "uuid-from-ta",
    "Arn": "arn:aws:kms:region:account:account/uuid",
    "CreationDate": "timestamp",
    "Enabled": true,
    "Description": "description",
    "WalletType": "HD_BIP32",
    "HasMnemonic": true
  },
  "Mnemonic": "twelve word mnemonic from TA"
}
```

#### 2. TrentService.DescribeAccount → Local metadata
**Purpose**: Return account metadata (no TA call needed)
**Request**:
```json
{
  "AccountId": "uuid"
}
```
**Response**:
```json
{
  "AccountMetadata": {
    "AccountId": "uuid",
    "Arn": "arn:aws:kms:region:account:account/uuid",
    "CreationDate": "timestamp",
    "Enabled": true,
    "Description": "description",
    "WalletType": "HD_BIP32",
    "HasMnemonic": true
  }
}
```

#### 3. TrentService.ListAccounts → Local metadata list
**Purpose**: List all created accounts
**Request**: `{}`
**Response**:
```json
{
  "Accounts": [
    {
      "AccountId": "uuid1",
      "Arn": "arn:aws:kms:region:account:account/uuid1",
      "CreationDate": "timestamp",
      "Enabled": true,
      "Description": "description",
      "WalletType": "HD_BIP32",
      "HasMnemonic": true
    }
  ]
}
```

#### 4. TrentService.DeriveAddress → TA DeriveAddress
**Purpose**: Derive Ethereum address using TA HD derivation
**Request**:
```json
{
  "AccountId": "uuid",
  "DerivationPath": "m/44'/60'/0'/0/0"
}
```
**Response**:
```json
{
  "AccountId": "uuid",
  "Address": "0x742d35Cc6634C0532925a3b8D4C8C8C3bfBb1234",
  "DerivationPath": "m/44'/60'/0'/0/0",
  "PublicKey": "base64-encoded-public-key-from-ta"
}
```

#### 5. TrentService.SignTransaction → TA SignTransaction
**Purpose**: Sign Ethereum transaction using TA
**Request**:
```json
{
  "AccountId": "uuid",
  "DerivationPath": "m/44'/60'/0'/0/0",
  "Transaction": {
    "chainId": 1,
    "nonce": 42,
    "to": "0x742d35Cc6634C0532925a3b8D4C8C8C3bfBb1234",
    "value": "1000000000000000000",
    "gasPrice": "20000000000",
    "gas": 21000,
    "data": "0x"
  }
}
```
**Response**:
```json
{
  "AccountId": "uuid",
  "Signature": "base64-encoded-signature-from-ta",
  "TransactionHash": "computed-hash",
  "RawTransaction": "rlp-encoded-signed-tx"
}
```

#### 6. TrentService.RemoveAccount → TA RemoveWallet
**Purpose**: Securely delete account from TEE storage
**Request**:
```json
{
  "AccountId": "uuid"
}
```
**Response**:
```json
{
  "AccountId": "uuid",
  "Removed": true
}
```

## Implementation Architecture

### Host Application (kms-api):
- **Role**: HTTP server, AWS KMS API compatibility layer
- **Responsibilities**:
  - Parse AWS KMS requests
  - Call eth_wallet TA via OP-TEE Client API
  - Format responses as AWS KMS compatible JSON
  - Store account metadata (non-sensitive)

### TA Integration:
- **Use existing eth_wallet TA** (no modifications)
- **Call TA via OP-TEE Client API**
- **Secure storage handled by TA** (not host)

### Data Flow:
```
AWS KMS Request → Host Parser → TA Call → TA Response → AWS KMS Response
```

## Security Model

### 🔒 TEE-Protected Data:
- Private keys (never leave TEE)
- Mnemonic phrases (only returned once on creation)
- HD derivation (performed in TEE)
- Transaction signing (performed in TEE)

### 🌐 Host-Stored Data:
- Account metadata (UUIDs, descriptions, creation dates)
- API compatibility mappings
- Non-sensitive configuration

## Deployment Requirements

### 1. TEE Environment:
- OP-TEE on ARM64 (QEMU for development)
- eth_wallet TA deployed and running
- Secure storage available

### 2. Host Application:
- Rust application with OP-TEE Client API
- HTTP server for AWS KMS compatibility
- No independent crypto implementations

## Testing Strategy

### 1. TA Integration Testing:
Test direct TA calls in QEMU environment

### 2. API Compatibility Testing:
Test AWS KMS compatible endpoints

### 3. Security Validation:
Verify that no crypto operations occur outside TEE

## Migration from Current Implementation

### ❌ Remove (Non-TA implementations):
- Independent secp256k1 usage
- Independent bip39 usage
- Independent keccak usage
- All simple_kms.rs implementations

### ✅ Keep (TA-based only):
- Account metadata management
- AWS KMS API parsing
- HTTP server infrastructure

---

**Key Principle**: If it's not in eth_wallet TA, we don't implement it. Period.