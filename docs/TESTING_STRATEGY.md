# AirAccount 测试策略文档

## 测试概述

本文档描述 AirAccount 系统的完整测试策略，包括单元测试、集成测试和端到端测试。

## 测试目标

### 核心验证目标
1. **SDK → CA → TA → TEE** 完整调用链功能正常
2. **用户凭证自主控制**架构正确实现
3. **双 CA 架构**（Rust + Node.js）均可正常工作
4. **WebAuthn/Passkey** 集成功能完整
5. **TEE 硬件隔离**私钥安全有效

### 质量保证目标
- 功能覆盖率 >95%
- 性能响应时间 <500ms
- 错误恢复率 >99%
- 并发支持 50+ 用户

## 测试架构

### 测试分层

```
┌─────────────────────────────┐
│     端到端测试 (E2E)         │ ← 用户场景完整流程
├─────────────────────────────┤
│      集成测试               │ ← SDK-CA-TA-TEE 集成
├─────────────────────────────┤
│      组件测试               │ ← CA/TA/SDK 独立测试
├─────────────────────────────┤
│      单元测试               │ ← 函数级别测试
└─────────────────────────────┘
```

### 测试环境

#### 1. 开发环境
- **QEMU TEE**: 模拟 TEE 硬件环境
- **本地 CA**: localhost:3001(Rust), localhost:3002(Node.js)
- **Mock 数据**: 测试用户和钱包数据

#### 2. 集成环境
- **真实 TEE**: Raspberry Pi 5 + OP-TEE
- **容器化 CA**: Docker 部署
- **真实网络**: HTTPS 连接

#### 3. 生产环境
- **硬件集群**: 多节点 TEE 集群
- **负载均衡**: 高可用 CA 服务
- **监控完整**: 实时性能监控

## 测试用例设计

### 1. 单元测试

#### SDK 单元测试
```typescript
// packages/node-sdk/tests/unit/
describe('AirAccountSDK', () => {
  test('should initialize successfully', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    await expect(sdk.initialize()).resolves.not.toThrow();
  });
  
  test('should handle invalid config', () => {
    expect(() => new AirAccountSDK({})).toThrow('caBaseUrl is required');
  });
  
  test('should validate WebAuthn data', () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    expect(() => sdk.registerWithWebAuthn({})).toThrow('email is required');
  });
});
```

#### CA 单元测试
```rust
// packages/airaccount-ca-extended/tests/
#[tokio::test]
async fn test_webauthn_challenge_generation() {
    let service = WebAuthnService::new();
    let challenge = service.generate_challenge("test@example.com").await;
    assert!(challenge.is_ok());
    assert_eq!(challenge.unwrap().len(), 32);
}

#[tokio::test]
async fn test_tee_connection() {
    let client = TeecClient::new();
    let result = client.test_connection().await;
    assert!(result.is_ok());
}
```

#### TA 单元测试
```rust
// packages/airaccount-ta-simple/tests/
fn test_wallet_creation() {
    let params = Parameters::empty();
    let result = create_wallet(&mut params);
    assert_eq!(result, TeecResult::Success);
}

fn test_transaction_signing() {
    let wallet_id = 1;
    let tx_data = "test_transaction";
    let result = sign_transaction(wallet_id, tx_data);
    assert!(result.is_ok());
}
```

### 2. 集成测试

#### SDK-CA 集成测试
```typescript
// packages/airaccount-sdk-test/test-ca-integration.js
describe('SDK-CA Integration', () => {
  test('Rust CA integration', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    await sdk.initialize();
    
    const registration = await sdk.registerWithWebAuthn({
      email: 'test-rust@example.com',
      displayName: 'Test User'
    });
    expect(registration.success).toBe(true);
    
    const wallet = await sdk.createAccount();
    expect(wallet.address).toMatch(/^0x[a-fA-F0-9]{40}$/);
  });
  
  test('Node.js CA integration', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3002' });
    // 同样的测试流程
  });
});
```

#### CA-TA 集成测试
```rust
// packages/airaccount-ca-extended/tests/integration/
#[tokio::test]
async fn test_ca_ta_communication() {
    let ca_service = CAService::new();
    let result = ca_service.create_wallet("test@example.com").await;
    assert!(result.is_ok());
    
    let wallet_info = result.unwrap();
    assert!(!wallet_info.address.is_empty());
    assert!(wallet_info.wallet_id > 0);
}
```

### 3. 端到端测试

#### 完整用户流程
```javascript
// packages/airaccount-sdk-test/e2e/user-journey.test.js
describe('Complete User Journey', () => {
  test('New user registration and wallet usage', async () => {
    // 1. 用户注册
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    await sdk.initialize();
    
    const registration = await sdk.registerWithWebAuthn({
      email: 'newuser@example.com',
      displayName: 'New User'
    });
    expect(registration.success).toBe(true);
    
    // 2. 创建钱包
    const wallet = await sdk.createAccount();
    expect(wallet.id).toBeGreaterThan(0);
    expect(wallet.address).toMatch(/^0x[a-fA-F0-9]{40}$/);
    
    // 3. 查询余额
    const balance = await sdk.getBalance();
    expect(balance.balance.eth).toBeDefined();
    
    // 4. 执行转账
    const transfer = await sdk.transfer({
      to: '0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A',
      amount: '0.01'
    });
    expect(transfer.txHash).toMatch(/^0x[a-fA-F0-9]{64}$/);
    
    // 5. 验证转账后余额
    const newBalance = await sdk.getBalance();
    expect(parseFloat(newBalance.balance.eth)).toBeLessThan(parseFloat(balance.balance.eth));
  });
  
  test('User session management', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    
    // 登录
    const auth = await sdk.authenticateWithWebAuthn({
      email: 'existing@example.com'
    });
    expect(auth.success).toBe(true);
    expect(sdk.isAuthenticated()).toBe(true);
    
    // 操作钱包
    const wallets = await sdk.listWallets();
    expect(wallets.length).toBeGreaterThan(0);
    
    // 登出
    sdk.logout();
    expect(sdk.isAuthenticated()).toBe(false);
  });
});
```

### 4. 性能测试

#### 并发测试
```javascript
// tests/performance/concurrent-users.test.js
describe('Performance Tests', () => {
  test('50 concurrent users', async () => {
    const promises = [];
    
    for (let i = 0; i < 50; i++) {
      promises.push(createUserAndWallet(`user${i}@example.com`));
    }
    
    const startTime = Date.now();
    const results = await Promise.all(promises);
    const endTime = Date.now();
    
    // 所有操作成功
    expect(results.every(r => r.success)).toBe(true);
    
    // 平均响应时间 < 1秒
    const avgTime = (endTime - startTime) / 50;
    expect(avgTime).toBeLessThan(1000);
  });
  
  test('TEE operation latency', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    await sdk.initialize();
    
    const operations = [];
    for (let i = 0; i < 10; i++) {
      const start = Date.now();
      await sdk.createAccount();
      const end = Date.now();
      operations.push(end - start);
    }
    
    const avgLatency = operations.reduce((a, b) => a + b) / operations.length;
    expect(avgLatency).toBeLessThan(500); // < 500ms
  });
});
```

#### 压力测试
```javascript
// tests/performance/stress.test.js
describe('Stress Tests', () => {
  test('High frequency requests', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    await sdk.initialize();
    
    // 快速连续请求
    const requests = [];
    for (let i = 0; i < 100; i++) {
      requests.push(sdk.getBalance());
    }
    
    const results = await Promise.allSettled(requests);
    const successCount = results.filter(r => r.status === 'fulfilled').length;
    
    // 成功率 > 95%
    expect(successCount / 100).toBeGreaterThan(0.95);
  });
});
```

### 5. 安全测试

#### 认证安全测试
```javascript
// tests/security/auth.test.js
describe('Security Tests', () => {
  test('Invalid session token rejection', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    sdk.sessionToken = 'invalid_token';
    
    await expect(sdk.createAccount()).rejects.toThrow('Authentication failed');
  });
  
  test('Session timeout handling', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    
    // 模拟会话过期
    await sdk.authenticateWithWebAuthn({ email: 'test@example.com' });
    
    // 等待会话过期
    await new Promise(resolve => setTimeout(resolve, 60000));
    
    await expect(sdk.createAccount()).rejects.toThrow('Session expired');
  });
});
```

#### 输入验证测试
```javascript
// tests/security/validation.test.js
describe('Input Validation', () => {
  test('SQL injection prevention', async () => {
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    
    const maliciousEmail = "'; DROP TABLE users; --";
    await expect(sdk.registerWithWebAuthn({
      email: maliciousEmail,
      displayName: 'Test'
    })).rejects.toThrow('Invalid email format');
  });
  
  test('XSS prevention', async () => {
    const script = '<script>alert("xss")</script>';
    const sdk = new AirAccountSDK({ caBaseUrl: 'http://localhost:3001' });
    
    await expect(sdk.registerWithWebAuthn({
      email: 'test@example.com',
      displayName: script
    })).rejects.toThrow('Invalid display name');
  });
});
```

## 测试自动化

### 持续集成 (CI)

```yaml
# .github/workflows/test.yml
name: AirAccount Tests

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup Node.js
        uses: actions/setup-node@v3
        with:
          node-version: '18'
      - name: Install dependencies
        run: npm install
      - name: Run unit tests
        run: npm run test:unit
        
  integration-tests:
    runs-on: ubuntu-latest
    services:
      qemu-tee:
        image: airaccout/qemu-tee:latest
        ports:
          - 8080:8080
    steps:
      - name: Start CA services
        run: |
          cargo run -p airaccount-ca-extended --bin ca-server &
          cd packages/airaccount-ca-nodejs && npm run dev &
      - name: Run integration tests
        run: npm run test:integration
        
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - name: Setup complete environment
        run: ./scripts/setup-test-env.sh
      - name: Run E2E tests
        run: npm run test:e2e
```

### 测试报告生成

```javascript
// scripts/generate-test-report.js
const fs = require('fs');
const path = require('path');

async function generateTestReport() {
  const results = {
    timestamp: new Date().toISOString(),
    environment: process.env.NODE_ENV || 'development',
    summary: {
      total: 0,
      passed: 0,
      failed: 0,
      coverage: 0
    },
    details: {
      unit: await runUnitTests(),
      integration: await runIntegrationTests(),
      e2e: await runE2ETests(),
      performance: await runPerformanceTests(),
      security: await runSecurityTests()
    }
  };
  
  // 生成报告
  const reportPath = path.join(__dirname, '../test-reports');
  if (!fs.existsSync(reportPath)) {
    fs.mkdirSync(reportPath, { recursive: true });
  }
  
  fs.writeFileSync(
    path.join(reportPath, `test-report-${Date.now()}.json`),
    JSON.stringify(results, null, 2)
  );
  
  // 生成 HTML 报告
  generateHTMLReport(results);
}
```

## 测试数据管理

### 测试用户数据
```json
{
  "testUsers": [
    {
      "email": "test-rust@airaccount.dev",
      "displayName": "Rust Test User",
      "caType": "rust",
      "wallets": [
        {
          "id": 1,
          "address": "0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A",
          "balance": "1.0"
        }
      ]
    },
    {
      "email": "test-nodejs@airaccount.dev", 
      "displayName": "Node.js Test User",
      "caType": "nodejs",
      "wallets": []
    }
  ]
}
```

### 测试钱包数据
```json
{
  "testWallets": [
    {
      "id": 1,
      "address": "0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A",
      "privateKey": "TEE_MANAGED",
      "balance": {
        "eth": "1.0",
        "tokens": {
          "USDC": "100.0"
        }
      }
    }
  ]
}
```

## 测试工具

### 自动化测试脚本

1. **`run-complete-test.sh`** - 一键完整测试
2. **`quick-test-sdk-ca.sh`** - 快速连接测试  
3. **`test-complete-integration.sh`** - 集成测试
4. **`performance-benchmark.sh`** - 性能基准测试

### 测试辅助工具

1. **测试数据生成器** - 生成模拟用户和钱包
2. **TEE 模拟器** - 本地 TEE 环境模拟
3. **API 监控器** - 实时 API 性能监控
4. **日志分析器** - 测试日志自动分析

## 质量标准

### 代码覆盖率要求
- **单元测试**: >90%
- **集成测试**: >80%
- **E2E 测试**: >70%
- **总体覆盖率**: >85%

### 性能基准
- **SDK 初始化**: <200ms
- **WebAuthn 操作**: <1000ms (包含用户交互)
- **TEE 操作**: <500ms
- **API 响应**: <300ms

### 可靠性指标
- **测试通过率**: >99%
- **并发成功率**: >95%
- **错误恢复率**: >99%
- **服务可用性**: >99.9%

---

*本文档版本: v1.0.0*  
*最后更新: 2025-01-15*  
*维护者: AirAccount Team*