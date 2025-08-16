#!/usr/bin/env node

/**
 * 基础测试服务器 - 验证核心架构
 * 跳过复杂的TypeScript编译，直接测试核心功能
 */

import express from 'express';
import cors from 'cors';
import { v4 as uuidv4 } from 'uuid';

const app = express();
const PORT = 3002;

// 中间件
app.use(cors());
app.use(express.json());

// 简单的内存存储
const users = new Map();
const sessions = new Map();
const challenges = new Map();

// 模拟TEE客户端
class MockTEEClient {
  async createAccountWithPasskey(email, credentialId, publicKey) {
    const walletId = Math.floor(Math.random() * 1000) + 1;
    const address = `0x${Math.random().toString(16).substring(2, 42).padStart(40, '0')}`;
    
    console.log(`🔐 TEE: Created hybrid account for ${email}`);
    console.log(`   Wallet ID: ${walletId}`);
    console.log(`   Address: ${address}`);
    
    return {
      walletId,
      ethereumAddress: address,
      teeDeviceId: `hybrid_account_${walletId}`,
    };
  }

  async verifySecurityState() {
    return {
      verified: true,
      status: 'security_state:VERIFIED,tee_entropy:PASS,memory_protection:PASS',
      details: {
        security_state: 'VERIFIED',
        tee_entropy: 'PASS',
        memory_protection: 'PASS',
        hybrid_entropy: 'PASS',
      },
    };
  }

  async testConnection() {
    return 'Mock TEE connection - Hello from AirAccount Mock TA with Hybrid Entropy';
  }

  async signTransaction(walletId, transactionData) {
    const signature = `0x${Math.random().toString(16).substring(2, 130)}`;
    const txHash = `0x${Math.random().toString(16).substring(2, 66)}`;
    
    console.log(`✍️ TEE: Signed transaction for wallet ${walletId}`);
    console.log(`   Signature: ${signature.substring(0, 20)}...`);
    
    return {
      transactionHash: txHash,
      signature,
      walletId,
    };
  }
}

const teeClient = new MockTEEClient();

// 路由：健康检查
app.get('/health', async (req, res) => {
  try {
    const teeConnection = await teeClient.testConnection();
    const securityState = await teeClient.verifySecurityState();
    
    res.json({
      status: 'OK',
      timestamp: new Date().toISOString(),
      teeConnection,
      database: 'Mock database connected',
      security: securityState,
      services: {
        webauthn: 'Available',
        hybridEntropy: 'Available',
        teeIntegration: 'Available',
      }
    });
  } catch (error) {
    res.status(500).json({
      status: 'ERROR',
      error: error.message,
    });
  }
});

// 路由：开始 WebAuthn 注册
app.post('/webauthn/register/begin', async (req, res) => {
  try {
    const { email, displayName } = req.body;
    
    if (!email || !displayName) {
      return res.status(400).json({
        success: false,
        error: 'Email and displayName are required',
      });
    }

    const userId = Buffer.from(email).toString('base64');
    const challenge = Buffer.from(Math.random().toString()).toString('base64');
    const sessionId = uuidv4();

    // 存储challenge和会话
    challenges.set(challenge, {
      userId,
      timestamp: Date.now(),
      expiresAt: Date.now() + 5 * 60 * 1000, // 5分钟过期
    });

    sessions.set(sessionId, {
      userId,
      email,
      isAuthenticated: false,
      createdAt: Date.now(),
    });

    console.log(`🔐 Starting WebAuthn registration for: ${email}`);

    res.json({
      success: true,
      sessionId,
      options: {
        challenge,
        rp: {
          name: 'AirAccount Test',
          id: 'localhost',
        },
        user: {
          id: userId,
          name: email,
          displayName,
        },
        pubKeyCredParams: [
          { alg: -7, type: 'public-key' }, // ES256
          { alg: -257, type: 'public-key' }, // RS256
        ],
        authenticatorSelection: {
          residentKey: 'preferred',
          userVerification: 'preferred',
          authenticatorAttachment: 'platform',
        },
        attestation: 'none',
      },
      notice: {
        userResponsibility: "重要：您的Passkey凭证将存储在您的设备中，请确保设备安全。节点不保存您的私钥凭证。",
        architecture: "client-controlled-credentials"
      }
    });

  } catch (error) {
    console.error('Registration begin failed:', error);
    res.status(400).json({
      success: false,
      error: error.message,
    });
  }
});

// 路由：完成 WebAuthn 注册
app.post('/webauthn/register/finish', async (req, res) => {
  try {
    const { email, registrationResponse, challenge } = req.body;
    
    if (!email || !registrationResponse || !challenge) {
      return res.status(400).json({
        success: false,
        error: 'Missing required fields',
      });
    }

    // 验证challenge
    const challengeData = challenges.get(challenge);
    if (!challengeData || Date.now() > challengeData.expiresAt) {
      return res.status(400).json({
        success: false,
        error: 'Invalid or expired challenge',
      });
    }

    const userId = Buffer.from(email).toString('base64');

    // 简化验证 - 在实际应用中需要完整的WebAuthn验证
    console.log(`✅ Registration verification for: ${email}`);
    console.log(`   Credential ID: ${registrationResponse.id}`);

    // 创建TEE钱包
    const walletResult = await teeClient.createAccountWithPasskey(
      email,
      registrationResponse.id,
      Buffer.from('mock_public_key')
    );

    // 存储用户信息
    users.set(userId, {
      email,
      credentialId: registrationResponse.id,
      walletId: walletResult.walletId,
      ethereumAddress: walletResult.ethereumAddress,
      createdAt: Date.now(),
    });

    // 清理challenge
    challenges.delete(challenge);

    console.log(`🎉 Registration completed for ${email}, wallet ID: ${walletResult.walletId}`);

    res.json({
      success: true,
      walletResult,
      userInstructions: {
        credentialId: registrationResponse.id,
        message: "请保存您的凭证ID和email，这是恢复钱包访问的重要信息",
        recoveryInfo: {
          email: email,
          credentialId: registrationResponse.id,
          walletId: walletResult.walletId,
          ethereumAddress: walletResult.ethereumAddress,
        },
        warning: "节点可能不可用，请将恢复信息保存在安全位置"
      }
    });

  } catch (error) {
    console.error('Registration finish failed:', error);
    res.status(400).json({
      success: false,
      error: error.message,
    });
  }
});

// 路由：开始 WebAuthn 认证
app.post('/webauthn/authenticate/begin', async (req, res) => {
  try {
    const { email } = req.body;
    
    const challenge = Buffer.from(Math.random().toString()).toString('base64');
    const userId = email ? Buffer.from(email).toString('base64') : undefined;

    // 存储challenge
    challenges.set(challenge, {
      userId: userId || '',
      timestamp: Date.now(),
      expiresAt: Date.now() + 5 * 60 * 1000, // 5分钟过期
    });

    console.log(`🔓 Starting WebAuthn authentication${email ? ' for: ' + email : ' (passwordless)'}`);

    res.json({
      success: true,
      options: {
        challenge,
        rpId: 'localhost',
        userVerification: 'preferred',
        allowCredentials: [], // 简化处理
      },
      notice: {
        passwordless: !email,
        message: email ? 
          "请使用您设备上的生物识别验证身份" : 
          "无密码模式：系统将根据您的凭证自动识别身份"
      }
    });

  } catch (error) {
    console.error('Authentication begin failed:', error);
    res.status(400).json({
      success: false,
      error: error.message,
    });
  }
});

// 路由：完成 WebAuthn 认证
app.post('/webauthn/authenticate/finish', async (req, res) => {
  try {
    const { email, authenticationResponse, challenge } = req.body;
    
    if (!email || !authenticationResponse || !challenge) {
      return res.status(400).json({
        success: false,
        error: 'Missing required fields',
      });
    }

    // 验证challenge
    const challengeData = challenges.get(challenge);
    if (!challengeData || Date.now() > challengeData.expiresAt) {
      return res.status(400).json({
        success: false,
        error: 'Invalid or expired challenge',
      });
    }

    const userId = Buffer.from(email).toString('base64');
    const user = users.get(userId);

    if (!user) {
      return res.status(400).json({
        success: false,
        error: 'User not found',
      });
    }

    // 简化验证 - 在实际应用中需要完整的WebAuthn验证
    console.log(`🔍 Authentication verification for: ${email}`);

    // 创建认证会话
    const sessionId = uuidv4();
    sessions.set(sessionId, {
      userId,
      email,
      isAuthenticated: true,
      authenticatedAt: Date.now(),
      expiresAt: Date.now() + 3600 * 1000, // 1小时
    });

    // 清理challenge
    challenges.delete(challenge);

    console.log(`✅ Authentication successful for ${email}`);

    res.json({
      success: true,
      sessionId,
      userAccount: {
        email,
        userId,
        walletId: user.walletId,
        ethereumAddress: user.ethereumAddress,
      },
      sessionInfo: {
        expiresIn: 3600,
        message: "会话已创建，您可以访问钱包功能"
      }
    });

  } catch (error) {
    console.error('Authentication finish failed:', error);
    res.status(400).json({
      success: false,
      error: error.message,
    });
  }
});

// 路由：验证TEE安全状态
app.get('/webauthn/security/verify', async (req, res) => {
  try {
    const securityState = await teeClient.verifySecurityState();

    res.json({
      success: true,
      securityState,
      notice: "这是TEE环境的实时安全状态验证"
    });

  } catch (error) {
    console.error('Security verification failed:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to verify security state',
    });
  }
});

// 路由：获取统计信息
app.get('/webauthn/stats', async (req, res) => {
  try {
    const stats = {
      totalUsers: users.size,
      totalDevices: users.size, // 简化：每个用户一个设备
      activeChallenges: challenges.size,
      activeSessions: sessions.size,
    };

    res.json({
      success: true,
      stats,
      disclaimer: "这些是临时统计信息，不代表用户资产安全性"
    });

  } catch (error) {
    console.error('Stats failed:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to get statistics',
    });
  }
});

// 路由：钱包操作 - 签名交易
app.post('/wallet/sign', async (req, res) => {
  try {
    const { sessionId, transactionData } = req.body;
    
    if (!sessionId || !transactionData) {
      return res.status(400).json({
        success: false,
        error: 'Session ID and transaction data are required',
      });
    }

    // 验证会话
    const session = sessions.get(sessionId);
    if (!session || !session.isAuthenticated || Date.now() > session.expiresAt) {
      return res.status(401).json({
        success: false,
        error: 'Invalid or expired session',
      });
    }

    // 获取用户信息
    const user = users.get(session.userId);
    if (!user) {
      return res.status(404).json({
        success: false,
        error: 'User not found',
      });
    }

    // 使用TEE签名交易
    const signResult = await teeClient.signTransaction(user.walletId, transactionData);

    console.log(`💰 Transaction signed for user ${session.email}`);

    res.json({
      success: true,
      signResult,
      userInfo: {
        email: session.email,
        walletId: user.walletId,
        ethereumAddress: user.ethereumAddress,
      }
    });

  } catch (error) {
    console.error('Transaction signing failed:', error);
    res.status(500).json({
      success: false,
      error: error.message,
    });
  }
});

// 启动服务器
app.listen(PORT, () => {
  console.log(`🚀 AirAccount Node.js CA (Basic Test Server) running on port ${PORT}`);
  console.log(`📊 Health check: http://localhost:${PORT}/health`);
  console.log(`🔐 WebAuthn endpoints available:`);
  console.log(`   POST /webauthn/register/begin`);
  console.log(`   POST /webauthn/register/finish`);
  console.log(`   POST /webauthn/authenticate/begin`);
  console.log(`   POST /webauthn/authenticate/finish`);
  console.log(`   GET  /webauthn/security/verify`);
  console.log(`   GET  /webauthn/stats`);
  console.log(`💰 Wallet endpoints:`);
  console.log(`   POST /wallet/sign`);
  console.log(`\n✨ Ready for testing with: node test-webauthn-complete-flow.js\n`);

  // 清理定时器
  setInterval(() => {
    const now = Date.now();
    for (const [key, data] of challenges.entries()) {
      if (now > data.expiresAt) {
        challenges.delete(key);
      }
    }
    for (const [key, data] of sessions.entries()) {
      if (data.expiresAt && now > data.expiresAt) {
        sessions.delete(key);
      }
    }
  }, 60000); // 每分钟清理一次
});

// 优雅关闭
process.on('SIGINT', () => {
  console.log('\n🛑 Shutting down server gracefully...');
  process.exit(0);
});

export default app;