/**
 * AirAccount Demo - 完整流程演示
 * 
 * 模拟真实前端应用的使用流程
 * Demo → SDK → CA → TA → QEMU TEE
 */

import { AirAccountSDKSimulator } from './test-ca-integration.js';
import crypto from 'crypto';

class AirAccountDemo {
  constructor() {
    this.users = [];
    this.currentUser = null;
  }

  log(message, level = 'info') {
    const prefix = {
      'info': '🎭',
      'success': '🎉',
      'error': '💥',
      'warn': '⚠️',
      'user': '👤'
    }[level];
    console.log(`${prefix} [DEMO] ${message}`);
  }

  // 模拟用户注册流程
  async registerUser(email, displayName, caType = 'rust') {
    this.log(`用户注册: ${email} (使用 ${caType.toUpperCase()} CA)`);
    
    const sdk = new AirAccountSDKSimulator({ ca: caType });
    
    try {
      // 1. 初始化SDK
      await sdk.initialize();
      
      // 2. 开始WebAuthn注册（模拟浏览器Passkey创建）
      this.log('模拟浏览器Passkey创建...');
      const webauthnOptions = await sdk.registerWithWebAuthn({
        email: email,
        displayName: displayName
      });
      
      // 3. 模拟用户完成生物识别验证
      this.log('模拟用户生物识别验证 (Face ID/Touch ID)...');
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      // 4. 创建钱包账户
      this.log('在TEE硬件中创建私钥...');
      const passkeyData = {
        credentialId: `demo_credential_${Date.now()}_${crypto.randomBytes(8).toString('hex')}`,
        publicKeyBase64: Buffer.from(`demo_public_key_${Date.now()}`).toString('base64')
      };
      
      const account = await sdk.createAccount({
        email: email,
        displayName: displayName
      }, passkeyData);
      
      // 5. 保存用户信息（模拟客户端存储）
      const user = {
        id: crypto.randomUUID(),
        email: email,
        displayName: displayName,
        caType: caType,
        account: account,
        passkeyData: passkeyData,
        sdk: sdk,
        createdAt: new Date().toISOString()
      };
      
      this.users.push(user);
      this.currentUser = user;
      
      this.log(`用户注册成功！`, 'success');
      this.log(`钱包地址: ${account.ethereum_address || account.walletResult?.ethereumAddress}`, 'user');
      
      return user;
      
    } catch (error) {
      this.log(`用户注册失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 模拟用户登录流程
  async loginUser(email) {
    this.log(`用户登录: ${email}`);
    
    const user = this.users.find(u => u.email === email);
    if (!user) {
      throw new Error('用户不存在，请先注册');
    }
    
    try {
      // 模拟WebAuthn认证流程
      this.log('模拟WebAuthn认证...');
      await new Promise(resolve => setTimeout(resolve, 500));
      
      this.currentUser = user;
      this.log(`用户登录成功！`, 'success');
      this.log(`欢迎回来，${user.displayName}`, 'user');
      
      return user;
      
    } catch (error) {
      this.log(`用户登录失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 查看钱包余额
  async checkBalance() {
    if (!this.currentUser) {
      throw new Error('请先登录');
    }
    
    this.log('查询钱包余额...');
    
    try {
      const walletId = this.currentUser.account.wallet_id || this.currentUser.account.walletResult?.walletId;
      const balance = await this.currentUser.sdk.getBalance(walletId);
      
      this.log('余额查询成功！', 'success');
      return balance;
      
    } catch (error) {
      this.log(`余额查询失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 发送转账
  async sendTransfer(toAddress, amount) {
    if (!this.currentUser) {
      throw new Error('请先登录');
    }
    
    this.log(`发起转账: ${amount} ETH → ${toAddress}`);
    
    try {
      // 模拟用户确认转账
      this.log('请确认转账并完成生物识别验证...');
      await new Promise(resolve => setTimeout(resolve, 1500));
      
      const walletId = this.currentUser.account.wallet_id || this.currentUser.account.walletResult?.walletId;
      const result = await this.currentUser.sdk.transfer(walletId, toAddress, amount);
      
      this.log('转账成功！', 'success');
      this.log(`交易已提交到区块链`, 'user');
      
      return result;
      
    } catch (error) {
      this.log(`转账失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 查看所有钱包
  async listWallets() {
    if (!this.currentUser) {
      throw new Error('请先登录');
    }
    
    this.log('获取钱包列表...');
    
    try {
      const wallets = await this.currentUser.sdk.listWallets();
      this.log('钱包列表获取成功！', 'success');
      return wallets;
      
    } catch (error) {
      this.log(`钱包列表获取失败: ${error.message}`, 'error');
      throw error;
    }
  }

  // 显示用户恢复信息
  showRecoveryInfo() {
    if (!this.currentUser) {
      throw new Error('请先登录');
    }
    
    this.log('显示钱包恢复信息...', 'user');
    console.log('📋 恢复信息 (请妥善保存):');
    console.log('   Email:', this.currentUser.email);
    console.log('   Passkey凭证ID:', this.currentUser.passkeyData.credentialId);
    console.log('   钱包地址:', this.currentUser.account.ethereum_address || this.currentUser.account.walletResult?.ethereumAddress);
    console.log('   CA类型:', this.currentUser.caType.toUpperCase());
    console.log('');
    console.log('⚠️  重要提醒:');
    console.log('   - 您的Passkey存储在设备的安全硬件中');
    console.log('   - 私钥存储在TEE硬件中，通过Passkey授权访问');
    console.log('   - 即使节点"跑路"，您仍可在其他兼容节点恢复钱包');
    console.log('   - 请备份上述恢复信息到安全位置');
  }
}

// Demo应用主流程
async function runDemo() {
  console.log('🎭 AirAccount 完整流程演示');
  console.log('='.repeat(60));
  console.log('模拟场景: 真实用户使用AirAccount硬件钱包');
  console.log('技术栈: Demo → SDK → CA → TA → QEMU TEE');
  console.log('');

  const demo = new AirAccountDemo();

  try {
    // 场景1: 新用户注册 (Rust CA)
    console.log('📱 场景1: 新用户注册 (Rust CA)');
    console.log('-'.repeat(40));
    
    const user1 = await demo.registerUser(
      'alice@example.com',
      'Alice Johnson',
      'rust'
    );
    
    // 显示恢复信息
    demo.showRecoveryInfo();
    
    console.log('\n');
    
    // 场景2: 钱包操作
    console.log('💰 场景2: 钱包操作');
    console.log('-'.repeat(40));
    
    await demo.checkBalance();
    await demo.listWallets();
    
    await demo.sendTransfer(
      '0x742d35Cc6609FD3eE86c7638F0AF8e08a2b6C44A',
      '0.1'
    );
    
    console.log('\n');
    
    // 场景3: 新用户注册 (Node.js CA)
    console.log('📱 场景3: 另一用户注册 (Node.js CA)');
    console.log('-'.repeat(40));
    
    const user2 = await demo.registerUser(
      'bob@example.com',
      'Bob Smith',
      'nodejs'
    );
    
    console.log('\n');
    
    // 场景4: 用户切换
    console.log('🔄 场景4: 用户登录切换');
    console.log('-'.repeat(40));
    
    await demo.loginUser('alice@example.com');
    await demo.checkBalance();
    
    await demo.loginUser('bob@example.com');
    await demo.checkBalance();
    
    console.log('\n');
    console.log('🎉 完整流程演示成功！');
    console.log('');
    console.log('📊 演示总结:');
    console.log('✅ 用户注册: 双CA支持');
    console.log('✅ WebAuthn认证: 生物识别模拟');
    console.log('✅ 钱包创建: TEE硬件密钥');
    console.log('✅ 资产操作: 查询、转账');
    console.log('✅ 用户切换: 多用户支持');
    console.log('✅ 恢复信息: 用户自主控制');
    console.log('');
    console.log('🔗 完整验证: Demo → SDK → 双CA → TA → QEMU TEE');
    console.log('🛡️  架构原则: 用户凭证自主控制，节点故障可恢复');
    
  } catch (error) {
    console.log(`\n💥 演示失败: ${error.message}`);
    console.log('请确保CA服务正在运行并且QEMU TEE环境可用');
  }
}

// 运行演示
if (import.meta.url === `file://${process.argv[1]}`) {
  runDemo().catch(console.error);
}