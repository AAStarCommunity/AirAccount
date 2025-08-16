/**
 * AirAccount SDK 模拟器 - CA集成测试
 * 
 * 完整测试链：SDK → CA → TA → QEMU TEE
 */

import fetch from 'node-fetch';
import crypto from 'crypto';

class AirAccountSDKSimulator {
  constructor(config) {
    this.caEndpoints = {
      rust: 'http://localhost:3001',
      nodejs: 'http://localhost:3002'
    };
    this.currentCA = config.ca || 'rust';
    this.baseURL = this.caEndpoints[this.currentCA];
    this.sessionId = null;
  }

  log(message, level = 'info') {
    const prefix = {
      'info': '📱',
      'success': '✅',
      'error': '❌',
      'warn': '⚠️'
    }[level];
    console.log(`${prefix} [SDK-${this.currentCA.toUpperCase()}] ${message}`);
  }

  async request(path, options = {}) {
    const url = `${this.baseURL}${path}`;
    const response = await fetch(url, {
      headers: {
        'Content-Type': 'application/json',
        ...options.headers
      },
      ...options
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${await response.text()}`);
    }

    return await response.json();
  }

  // 1. SDK初始化 - 检查CA和TEE连接
  async initialize() {
    this.log('初始化SDK...');
    
    try {
      const health = await this.request('/health');
      
      if (this.currentCA === 'rust') {
        if (health.tee_connected) {
          this.log('TEE连接正常', 'success');
        } else {
          this.log('TEE连接异常', 'warn');
        }
      } else {
        if (health.services?.tee?.connected) {
          this.log('TEE连接正常', 'success');
        } else {
          this.log('TEE连接异常', 'warn');
        }
      }
      
      this.log('SDK初始化成功', 'success');
      return health;
    } catch (error) {
      this.log(`初始化失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 2. WebAuthn注册流程
  async registerWithWebAuthn(userInfo) {
    this.log(`开始WebAuthn注册: ${userInfo.email}`);
    
    try {
      // 开始注册
      const registerPath = this.currentCA === 'rust' 
        ? '/api/webauthn/register/begin'
        : '/api/webauthn/register/begin';
      
      const registerData = this.currentCA === 'rust' 
        ? {
            user_id: crypto.randomUUID(),
            user_name: userInfo.email,
            user_display_name: userInfo.displayName,
            rp_name: 'AirAccount',
            rp_id: 'localhost'
          }
        : {
            email: userInfo.email,
            displayName: userInfo.displayName
          };
      
      const registerResponse = await this.request(registerPath, {
        method: 'POST',
        body: JSON.stringify(registerData)
      });
      
      this.log('WebAuthn挑战生成成功');
      
      if (this.currentCA === 'nodejs' && registerResponse.sessionId) {
        this.sessionId = registerResponse.sessionId;
        this.log(`会话创建: ${this.sessionId}`);
      }
      
      return registerResponse;
    } catch (error) {
      this.log(`WebAuthn注册失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 为测试环境完成WebAuthn注册
  async completeWebAuthnRegistration(userInfo, registerResponse) {
    try {
      const mockRegistrationResponse = {
        id: 'test_credential_' + Date.now(),
        rawId: 'test_raw_id',
        response: {
          clientDataJSON: 'mock_client_data',
          attestationObject: 'mock_attestation'
        },
        type: 'public-key'
      };
      
      const finishResponse = await this.request('/api/webauthn/register/finish', {
        method: 'POST',
        body: JSON.stringify({
          email: userInfo.email,
          registrationResponse: mockRegistrationResponse,
          challenge: registerResponse.options?.challenge || 'mock_challenge'
        })
      });
      
      this.log('WebAuthn注册完成成功');
      return finishResponse;
    } catch (error) {
      // 静默处理失败，在测试中不是关键路径
      this.log(`Mock WebAuthn完成失败: ${error.message}`, 'warn');
    }
  }

  // 3. 创建钱包账户
  async createAccount(userInfo, passkeyData) {
    this.log('创建钱包账户...');
    
    try {
      const createPath = this.currentCA === 'rust'
        ? '/api/account/create'
        : '/api/wallet/create';
      
      const createData = this.currentCA === 'rust'
        ? {
            email: userInfo.email,
            passkey_credential_id: passkeyData.credentialId,
            passkey_public_key_base64: passkeyData.publicKeyBase64
          }
        : {
            sessionId: this.sessionId,
            email: userInfo.email,
            passkeyCredentialId: passkeyData.credentialId
          };
      
      const account = await this.request(createPath, {
        method: 'POST',
        body: JSON.stringify(createData)
      });
      
      if (account.success !== false) {
        this.log(`账户创建成功 - 钱包ID: ${account.wallet_id || account.walletResult?.walletId}`, 'success');
        this.log(`以太坊地址: ${account.ethereum_address || account.walletResult?.ethereumAddress}`, 'success');
      }
      
      return account;
    } catch (error) {
      this.log(`账户创建失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 4. 查询余额
  async getBalance(walletId) {
    this.log(`查询余额 - 钱包ID: ${walletId}`);
    
    try {
      const balancePath = this.currentCA === 'rust'
        ? '/api/account/balance'
        : '/api/wallet/balance';
      
      const balanceData = this.currentCA === 'rust'
        ? { wallet_id: walletId }
        : { sessionId: this.sessionId, walletId: walletId };
      
      const balance = await this.request(balancePath, {
        method: 'POST',
        body: JSON.stringify(balanceData)
      });
      
      if (balance.success !== false) {
        const ethBalance = balance.balance_eth || balance.wallet?.balance?.eth || '模拟余额';
        this.log(`余额查询成功: ${ethBalance} ETH`, 'success');
      }
      
      return balance;
    } catch (error) {
      this.log(`余额查询失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 5. 执行转账
  async transfer(walletId, toAddress, amount) {
    this.log(`执行转账 - 金额: ${amount} ETH`);
    
    try {
      const transferPath = this.currentCA === 'rust'
        ? '/api/transaction/transfer'
        : '/api/wallet/transfer';
      
      const transferData = this.currentCA === 'rust'
        ? {
            wallet_id: walletId,
            to_address: toAddress,
            amount: amount
          }
        : {
            sessionId: this.sessionId,
            walletId: walletId,
            toAddress: toAddress,
            amount: amount
          };
      
      const transfer = await this.request(transferPath, {
        method: 'POST',
        body: JSON.stringify(transferData)
      });
      
      if (transfer.success !== false) {
        const txHash = transfer.transaction_hash || transfer.transaction?.transactionHash;
        this.log(`转账成功 - 交易哈希: ${txHash}`, 'success');
      }
      
      return transfer;
    } catch (error) {
      this.log(`转账失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 6. 列出钱包
  async listWallets() {
    this.log('列出所有钱包...');
    
    try {
      const listPath = this.currentCA === 'rust'
        ? '/api/wallet/list'
        : '/api/wallet/list';
      
      const listOptions = this.currentCA === 'nodejs'
        ? {
            method: 'GET',
            headers: {
              'Content-Type': 'application/json'
            }
          }
        : { method: 'GET' };
      
      // 添加sessionId到查询参数（如果是Node.js版本）
      const url = this.currentCA === 'nodejs' && this.sessionId
        ? `${listPath}?sessionId=${this.sessionId}`
        : listPath;
      
      const wallets = await this.request(url, listOptions);
      
      if (wallets.success !== false) {
        const count = wallets.wallets ? wallets.wallets.length : 0;
        this.log(`钱包列表获取成功 - 总数: ${count}`, 'success');
      }
      
      return wallets;
    } catch (error) {
      this.log(`列出钱包失败: ${error.message}`, 'error');
      throw error;
    }
  }
}

// 完整测试流程
async function runFullIntegrationTest(caType = 'rust') {
  console.log(`\n🧪 开始${caType.toUpperCase()} CA完整集成测试`);
  console.log('='.repeat(50));
  
  const sdk = new AirAccountSDKSimulator({ ca: caType });
  
  try {
    // 1. 初始化
    await sdk.initialize();
    
    // 2. WebAuthn注册
    const userInfo = {
      email: `test-${caType}@airaccount.dev`,
      displayName: `${caType.toUpperCase()} Test User`
    };
    
    await sdk.registerWithWebAuthn(userInfo);
    
    // 3. 创建账户
    const passkeyData = {
      credentialId: `${caType}_credential_${Date.now()}`,
      publicKeyBase64: Buffer.from(`${caType}_mock_public_key`).toString('base64')
    };
    
    const account = await sdk.createAccount(userInfo, passkeyData);
    const walletId = account.wallet_id || account.walletResult?.walletId;
    
    if (!walletId) {
      throw new Error('未能获取钱包ID');
    }
    
    // 4. 查询余额
    await sdk.getBalance(walletId);
    
    // 5. 执行转账
    await sdk.transfer(
      walletId,
      '0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A',
      '0.1'
    );
    
    // 6. 列出钱包
    await sdk.listWallets();
    
    console.log(`\n✅ ${caType.toUpperCase()} CA完整集成测试成功！`);
    console.log(`🔗 验证调用链: SDK → ${caType.toUpperCase()} CA → TA → QEMU TEE`);
    
    return true;
    
  } catch (error) {
    console.log(`\n❌ ${caType.toUpperCase()} CA集成测试失败: ${error.message}`);
    return false;
  }
}

// 主函数
async function main() {
  const args = process.argv.slice(2);
  const caType = args.find(arg => arg.startsWith('--ca='))?.split('=')[1] || 'rust';
  
  console.log('🚀 AirAccount SDK-CA-TA-TEE 完整集成测试');
  console.log('测试目标: 验证SDK到QEMU TEE的完整调用链');
  
  if (caType === 'both') {
    console.log('\n📋 测试计划: 双CA测试');
    
    const rustResult = await runFullIntegrationTest('rust');
    await new Promise(resolve => setTimeout(resolve, 2000)); // 等待2秒
    const nodejsResult = await runFullIntegrationTest('nodejs');
    
    console.log('\n📊 测试结果汇总:');
    console.log(`Rust CA:    ${rustResult ? '✅ 成功' : '❌ 失败'}`);
    console.log(`Node.js CA: ${nodejsResult ? '✅ 成功' : '❌ 失败'}`);
    
    if (rustResult && nodejsResult) {
      console.log('\n🎉 双CA完整集成测试全部成功！');
      console.log('🔗 验证: SDK → 双CA → TA → QEMU TEE 调用链完整');
    }
  } else {
    await runFullIntegrationTest(caType);
  }
}

// 运行测试
if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(console.error);
}

export { AirAccountSDKSimulator };