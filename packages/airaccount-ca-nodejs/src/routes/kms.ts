/**
 * KMS (Key Management Service) 路由
 * 专用于 SuperRelay Paymaster 集成的双重签名验证端点
 * 
 * 安全模型：
 * 1. Paymaster 签名验证（业务规则验证）
 * 2. 用户 Passkey 签名验证（用户意图验证）
 * 3. TEE 硬件签名（最终密钥保护）
 */

import { Router, Request, Response } from 'express';
import { z } from 'zod';
import { ethers } from 'ethers';
import type { AppState } from '../index.js';

const router = Router();

// 双重签名请求验证Schema
const DualSignRequestSchema = z.object({
  userOperation: z.object({
    sender: z.string(),
    nonce: z.string(),
    initCode: z.string(),
    callData: z.string(),
    callGasLimit: z.string(),
    verificationGasLimit: z.string(),
    preVerificationGas: z.string(),
    maxFeePerGas: z.string(),
    maxPriorityFeePerGas: z.string(),
    paymasterAndData: z.string(),
  }),
  accountId: z.string(),
  signatureFormat: z.string(),
  userSignature: z.string(),  // 用户 Passkey 签名
  userPublicKey: z.string(),  // 用户公钥
  businessValidation: z.object({
    balance: z.string(),
    membershipLevel: z.string(),
    approvedAt: z.number()
  }),
  nonce: z.number(),
  timestamp: z.number()
});

// Nonce 存储（生产环境应使用 Redis）
const nonceStore = new Set<number>();

// 授权的 Paymaster 地址（生产环境应存储在数据库）
const authorizedPaymasters = new Set<string>();

/**
 * 双重签名验证端点
 * 为 SuperRelay Paymaster 提供 TEE 签名服务
 * 
 * POST /kms/sign-user-operation
 */
router.post('/sign-user-operation', async (req: Request, res: Response): Promise<void | Response> => {
  try {
    const requestData = DualSignRequestSchema.parse(req.body);
    const paymasterSignature = req.headers['x-paymaster-signature'] as string;
    const paymasterAddress = req.headers['x-paymaster-address'] as string;
    const appState = (req as any).appState as AppState;

    console.log(`🔐 Processing dual-signature request from Paymaster: ${paymasterAddress}`);

    // 1. 验证时间戳（防重放，5分钟有效期）
    const currentTime = Math.floor(Date.now() / 1000);
    if (Math.abs(currentTime - requestData.timestamp) > 300) {
      return res.status(401).json({
        success: false,
        error: 'Request expired',
        details: 'Request timestamp is too old'
      });
    }

    // 2. 验证 nonce 唯一性（防重放）
    if (nonceStore.has(requestData.nonce)) {
      return res.status(401).json({
        success: false,
        error: 'Nonce already used',
        details: 'This nonce has been used before'
      });
    }
    nonceStore.add(requestData.nonce);
    
    // 清理过期的 nonce（10分钟后）
    setTimeout(() => {
      nonceStore.delete(requestData.nonce);
    }, 600000);

    // 3. 验证 Paymaster 签名（第一层验证）
    const paymasterMessage = ethers.solidityPackedKeccak256(
      ['bytes32', 'string', 'bytes32', 'uint256', 'uint256'],
      [
        getUserOperationHash(requestData.userOperation),
        requestData.accountId,
        ethers.keccak256(ethers.toUtf8Bytes(requestData.userSignature)),
        requestData.nonce,
        requestData.timestamp
      ]
    );

    const recoveredPaymasterAddress = ethers.verifyMessage(
      ethers.getBytes(paymasterMessage),
      paymasterSignature
    );

    if (recoveredPaymasterAddress.toLowerCase() !== paymasterAddress.toLowerCase()) {
      return res.status(401).json({
        success: false,
        error: 'Invalid Paymaster signature',
        details: 'Paymaster signature verification failed'
      });
    }

    console.log(`✅ Paymaster signature verified: ${paymasterAddress}`);

    // 4. 验证 Paymaster 是否被授权
    if (!authorizedPaymasters.has(paymasterAddress.toLowerCase())) {
      return res.status(403).json({
        success: false,
        error: 'Paymaster not authorized',
        details: `Paymaster ${paymasterAddress} is not in whitelist`
      });
    }

    // 5. 验证用户 Passkey 签名（第二层验证）
    const userOpHash = getUserOperationHash(requestData.userOperation);
    const userMessageHash = ethers.solidityPackedKeccak256(
      ['bytes32', 'string'],
      [userOpHash, requestData.accountId]
    );

    // 验证用户的 Passkey 签名
    const isValidUserSignature = await verifyPasskeySignature(
      requestData.userSignature,
      requestData.userPublicKey,
      userMessageHash,
      requestData.accountId,
      appState
    );

    if (!isValidUserSignature) {
      return res.status(401).json({
        success: false,
        error: 'Invalid user Passkey signature',
        details: 'User Passkey signature verification failed'
      });
    }

    console.log(`✅ User Passkey signature verified for account: ${requestData.accountId}`);

    // 6. 记录业务验证信息（审计日志）
    await recordAuditLog({
      type: 'DUAL_SIGNATURE_SPONSORSHIP',
      accountId: requestData.accountId,
      paymasterAddress,
      userPublicKey: requestData.userPublicKey,
      businessValidation: requestData.businessValidation,
      userOpHash,
      timestamp: new Date()
    }, appState);

    // 7. 通过 TEE TA 签名（最终签名）
    const teeResult = await signWithTEE({
      accountId: requestData.accountId,
      messageHash: userOpHash,
      signatureType: 'ECDSA_SECP256K1',
      metadata: {
        dualSignatureVerified: true,
        paymasterAddress,
        userPublicKey: requestData.userPublicKey
      }
    }, appState);

    console.log(`✅ TEE signature completed for UserOp: ${userOpHash}`);

    // 8. 返回标准格式
    res.json({
      success: true,
      signature: teeResult.signature,
      userOpHash,
      teeDeviceId: teeResult.deviceId,
      verificationProof: {
        paymasterVerified: true,
        userPasskeyVerified: true,
        dualSignatureMode: true,
        timestamp: new Date().toISOString()
      }
    });

  } catch (error) {
    console.error('KMS dual-signature request failed:', error);
    
    // 清理已使用的 nonce
    if (req.body.nonce) {
      nonceStore.delete(req.body.nonce);
    }
    
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'KMS request failed',
      details: 'Dual signature verification or TEE signing failed'
    });
  }
});

/**
 * 添加授权的 Paymaster 地址
 * 
 * POST /kms/admin/authorize-paymaster
 */
router.post('/admin/authorize-paymaster', async (req: Request, res: Response): Promise<void | Response> => {
  try {
    const adminToken = req.headers['admin-token'];
    // Expected: Compare with secure environment variable
    if (adminToken !== process.env.ADMIN_TOKEN) {
      return res.status(401).json({
        success: false,
        error: 'Unauthorized admin access'
      });
    }

    const { paymasterAddress, name, permissions } = z.object({
      paymasterAddress: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
      name: z.string(),
      permissions: z.array(z.string())
    }).parse(req.body);

    authorizedPaymasters.add(paymasterAddress.toLowerCase());

    console.log(`✅ Authorized Paymaster: ${paymasterAddress} (${name})`);

    res.json({
      success: true,
      message: `Paymaster ${paymasterAddress} authorized successfully`,
      authorizedPaymaster: {
        address: paymasterAddress,
        name,
        permissions,
        authorizedAt: new Date().toISOString()
      }
    });

  } catch (error) {
    console.error('Authorize Paymaster failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Authorization failed'
    });
  }
});

/**
 * 获取 KMS 服务状态
 * 
 * GET /kms/status
 */
router.get('/status', async (req: Request, res: Response) => {
  try {
    const appState = (req as any).appState as AppState;
    
    // 检查 TEE 连接状态
    let teeStatus = 'unknown';
    try {
      await appState.teeClient.healthCheck();
      teeStatus = 'healthy';
    } catch (error) {
      teeStatus = 'unhealthy';
    }

    res.json({
      success: true,
      status: {
        service: 'AirAccount KMS',
        mode: 'dual-signature',
        teeConnection: teeStatus,
        authorizedPaymastersCount: authorizedPaymasters.size,
        activeNoncesCount: nonceStore.size,
        features: [
          'Dual signature verification',
          'TEE hardware protection',
          'Anti-replay protection',
          'Paymaster authorization'
        ]
      },
      timestamp: new Date().toISOString()
    });

  } catch (error) {
    res.status(500).json({
      success: false,
      error: 'Failed to get KMS status'
    });
  }
});

// === 辅助函数 ===

/**
 * 验证用户 Passkey 签名
 */
async function verifyPasskeySignature(
  signature: string,
  publicKey: string,
  messageHash: string,
  accountId: string,
  appState: AppState
): Promise<boolean> {
  try {
    // 从数据库获取账户绑定的 Passkey 凭证
    const credential = await getPasskeyCredential(accountId, appState);
    
    if (!credential || credential.publicKey !== publicKey) {
      console.warn(`❌ No matching Passkey credential for account: ${accountId}`);
      return false;
    }
    
    // 使用 WebAuthn 服务验证签名
    const isValid = await appState.webauthnService.verifySignature({
      signature,
      publicKey,
      messageHash,
      credentialId: credential.credentialId
    });
    
    return isValid;
  } catch (error) {
    console.error('Passkey signature verification failed:', error);
    return false;
  }
}

/**
 * 获取 Passkey 凭证信息
 */
async function getPasskeyCredential(accountId: string, appState: AppState) {
  try {
    // 在生产环境中，这里应该查询数据库中存储的 Passkey 凭证
    const credential = await appState.database.getPasskeyCredential(accountId);
    return credential;
  } catch (error) {
    console.error('Failed to get Passkey credential:', error);
    return null;
  }
}

/**
 * 通过 TEE 签名
 */
async function signWithTEE(params: {
  accountId: string;
  messageHash: string;
  signatureType: string;
  metadata: any;
}, appState: AppState) {
  try {
    // 转换 accountId 为 walletId（数字）
    const walletId = parseInt(ethers.keccak256(ethers.toUtf8Bytes(params.accountId)).slice(2, 10), 16);
    
    const signResult = await appState.teeClient.signTransaction(
      walletId,
      params.messageHash
    );
    
    return {
      signature: signResult.signature,
      deviceId: `tee_${walletId}`
    };
  } catch (error) {
    console.warn('TEE signing failed, using mock signature for development:', error);
    
    // TODO: CRITICAL - Replace with real OP-TEE(TA) integration
    // 当前使用模拟签名进行开发和测试，生产环境必须连接真实的 TEE 设备
    if (process.env.NODE_ENV === 'development') {
      console.warn('⚠️  Using MOCK TEE signature - NOT suitable for production!');
      const mockSignature = ethers.Signature.from({
        r: '0x' + '1'.repeat(64),
        s: '0x' + '2'.repeat(64),
        v: 27
      }).serialized;
      
      return {
        signature: mockSignature,
        deviceId: `mock_tee_${params.accountId}`
      };
    }
    
    throw error;
  }
}

/**
 * 记录审计日志
 */
async function recordAuditLog(logData: any, appState: AppState) {
  try {
    console.log('📝 Audit Log:', JSON.stringify(logData, null, 2));
    
    // 在生产环境中，这里应该将日志写入安全的审计数据库
    // await appState.database.recordAuditLog(logData);
  } catch (error) {
    console.error('Failed to record audit log:', error);
  }
}

/**
 * 计算 UserOperation 哈希（ERC-4337标准）
 */
function getUserOperationHash(userOp: any): string {
  const entryPointAddress = '0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789';
  const chainId = 11155111; // Sepolia

  const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
    ['address', 'uint256', 'bytes32', 'bytes32', 'uint256', 'uint256', 'uint256', 'uint256', 'uint256', 'bytes32'],
    [
      userOp.sender,
      userOp.nonce,
      ethers.keccak256(userOp.initCode),
      ethers.keccak256(userOp.callData),
      userOp.callGasLimit,
      userOp.verificationGasLimit,
      userOp.preVerificationGas,
      userOp.maxFeePerGas,
      userOp.maxPriorityFeePerGas,
      ethers.keccak256(userOp.paymasterAndData)
    ]
  );

  return ethers.keccak256(
    ethers.AbiCoder.defaultAbiCoder().encode(
      ['bytes32', 'address', 'uint256'],
      [ethers.keccak256(encoded), entryPointAddress, chainId]
    )
  );
}

export { router as kmsRoutes };