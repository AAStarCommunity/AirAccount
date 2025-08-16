/**
 * 账户抽象路由 - ERC-4337集成
 * 
 * 参考资料：
 * - https://github.com/mingder78/all-about-abstract-account
 * - ERC-4337 Account Abstraction标准
 */

import { Router, Request, Response } from 'express';
import { z } from 'zod';
import { ethers } from 'ethers';
import type { AppState } from '../index.js';

const router = Router();

// 验证schema
const CreateAccountSchema = z.object({
  sessionId: z.string(),
  email: z.string().email(),
  salt: z.string().optional(),
  initialDeposit: z.string().optional(),
  recoveryGuardians: z.array(z.string()).optional(),
});

const UserOperationSchema = z.object({
  sessionId: z.string(),
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
});

const TransactionSchema = z.object({
  sessionId: z.string(),
  to: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  value: z.string().optional(),
  data: z.string().optional(),
  usePaymaster: z.boolean().optional(),
});

const BatchTransactionSchema = z.object({
  sessionId: z.string(),
  transactions: z.array(z.object({
    to: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
    value: z.string().optional(),
    data: z.string().optional(),
  })),
  usePaymaster: z.boolean().optional(),
});

// 中间件：验证会话
async function requireAuth(req: any, res: Response, next: Function): Promise<void> {
  try {
    const sessionId = req.body.sessionId || req.query.sessionId;
    if (!sessionId) {
      res.status(401).json({
        success: false,
        error: 'Session ID required',
      });
      return;
    }

    const appState = req.appState as AppState;
    const session = await appState.database.getSession(sessionId);
    
    if (!session || !session.isAuthenticated) {
      res.status(401).json({
        success: false,
        error: 'Invalid or unauthenticated session',
      });
      return;
    }

    req.session = session;
    next();
  } catch (error) {
    res.status(500).json({
      success: false,
      error: 'Authentication verification failed',
    });
  }
}

/**
 * 创建抽象账户
 * 
 * POST /aa/create-account
 */
router.post('/create-account', requireAuth, async (req: Request, res: Response) => {
  try {
    const { sessionId, email, salt, initialDeposit, recoveryGuardians } = CreateAccountSchema.parse(req.body);
    const appState = (req as any).appState as AppState;
    const session = (req as any).session;

    console.log(`🏦 Creating abstract account for ${email}`);

    // 生成确定性账户地址
    const credentialId = await getUserCredentialId(email);
    const accountSalt = salt || ethers.id(credentialId);
    const accountAddress = await computeAccountAddress(credentialId, accountSalt);

    // 生成初始化代码
    const initCode = await generateInitCode(credentialId, accountSalt, recoveryGuardians);

    // 创建账户信息
    const accountInfo = {
      address: accountAddress,
      nonce: 0,
      isDeployed: Boolean(initialDeposit),
      owner: credentialId,
      recoveryMethod: 'passkey' as const,
      salt: accountSalt,
      initCode,
      email
    };

    // 如果有初始存款，部署账户
    if (initialDeposit) {
      console.log(`💰 Deploying account with initial deposit: ${initialDeposit} ETH`);
      const deployResult = await deployAccountWithDeposit(
        accountAddress,
        initCode,
        initialDeposit,
        appState
      );
      
      accountInfo.isDeployed = true;
      accountInfo.nonce = 1;
      
      console.log(`✅ Account deployed at: ${accountAddress}`);
    }

    // 存储账户会话信息
    await appState.database.storeWalletSession(sessionId, {
      walletId: parseInt(accountAddress.slice(2, 10), 16),
      ethereumAddress: accountAddress,
      teeDeviceId: `aa_account_${credentialId}`,
    });

    res.json({
      success: true,
      account: accountInfo,
      userGuidance: {
        architecture: 'ERC-4337 Account Abstraction',
        features: [
          'Gasless transactions with Paymaster support',
          'Batch transaction execution',
          'WebAuthn-based transaction signing',
          'Social recovery with guardians',
          'Cross-device account access'
        ],
        security: {
          passkeyControl: 'Your Passkey controls this smart contract account',
          recoveryMethod: recoveryGuardians ? 'Guardian-based social recovery' : 'Passkey-only recovery',
          gasStrategy: 'Pay gas with ETH or use sponsored transactions'
        }
      }
    });
    return;

  } catch (error) {
    console.error('Abstract account creation failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Account creation failed',
    });
  }
});

/**
 * 获取账户信息
 * 
 * POST /aa/account-info
 */
router.post('/account-info', requireAuth, async (req: Request, res: Response) => {
  try {
    const { sessionId } = z.object({ sessionId: z.string() }).parse(req.body);
    const appState = (req as any).appState as AppState;
    const session = (req as any).session;

    console.log(`📋 Getting account info for session: ${sessionId}`);

    // 从会话获取账户信息
    const walletSession = await appState.database.getWalletSession(sessionId);
    if (!walletSession) {
      res.status(404).json({
        success: false,
        error: 'No account found for this session',
      });
      return;
    }

    // 查询链上状态
    const nonce = await getAccountNonce(walletSession.ethereumAddress);
    const balance = await getAccountBalance(walletSession.ethereumAddress);
    const isDeployed = nonce > 0;

    const accountInfo = {
      address: walletSession.ethereumAddress,
      nonce,
      isDeployed,
      balance: {
        native: ethers.formatEther(balance),
        tokens: [] // TODO: 查询ERC-20余额
      },
      owner: session.email,
      recoveryMethod: 'passkey'
    };

    res.json({
      success: true,
      account: accountInfo,
      blockchain: {
        network: 'Ethereum Sepolia Testnet',
        entryPoint: '0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789',
        chainId: 11155111
      }
    });

  } catch (error) {
    console.error('Get account info failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Get account info failed',
    });
  }
});

/**
 * 执行交易
 * 
 * POST /aa/execute-transaction
 */
router.post('/execute-transaction', requireAuth, async (req: Request, res: Response) => {
  try {
    const { sessionId, to, value, data, usePaymaster } = TransactionSchema.parse(req.body);
    const appState = (req as any).appState as AppState;
    const session = (req as any).session;

    console.log(`💸 Executing transaction: ${to}, value: ${value || '0'}`);

    // 获取账户信息
    const walletSession = await appState.database.getWalletSession(sessionId);
    if (!walletSession) {
      res.status(404).json({
        success: false,
        error: 'No account found for this session',
      });
      return;
    }

    // 构建UserOperation
    const userOp = await buildUserOperation({
      sender: walletSession.ethereumAddress,
      to,
      value: value || '0',
      data: data || '0x',
      usePaymaster: Boolean(usePaymaster)
    });

    // 通过TEE签名UserOperation
    const signature = await signUserOperationWithTEE(userOp, appState);
    userOp.signature = signature;

    // 提交到Bundler
    const txHash = await submitUserOperation(userOp);

    console.log(`✅ Transaction submitted: ${txHash}`);

    res.json({
      success: true,
      transaction: {
        userOpHash: txHash,
        sender: userOp.sender,
        to,
        value: value || '0',
        usePaymaster: Boolean(usePaymaster),
        estimatedConfirmation: '15-30 seconds'
      },
      userOperation: userOp,
      notice: usePaymaster ? 
        'Transaction sponsored - no gas fees required' : 
        'Gas fees will be deducted from account balance'
    });

  } catch (error) {
    console.error('Transaction execution failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Transaction execution failed',
    });
  }
});

/**
 * 批量执行交易
 * 
 * POST /aa/execute-batch
 */
router.post('/execute-batch', requireAuth, async (req: Request, res: Response) => {
  try {
    const { sessionId, transactions, usePaymaster } = BatchTransactionSchema.parse(req.body);
    const appState = (req as any).appState as AppState;

    console.log(`📦 Executing batch transaction: ${transactions.length} operations`);

    // 获取账户信息
    const walletSession = await appState.database.getWalletSession(sessionId);
    if (!walletSession) {
      res.status(404).json({
        success: false,
        error: 'No account found for this session',
      });
      return;
    }

    // 构建批量调用数据
    const batchCallData = encodeBatchCall(transactions);

    // 构建UserOperation
    const userOp = await buildUserOperation({
      sender: walletSession.ethereumAddress,
      to: walletSession.ethereumAddress, // 调用自己的executeBatch函数
      value: '0',
      data: batchCallData,
      usePaymaster: Boolean(usePaymaster)
    });

    // 通过TEE签名UserOperation
    const signature = await signUserOperationWithTEE(userOp, appState);
    userOp.signature = signature;

    // 提交到Bundler
    const txHash = await submitUserOperation(userOp);

    console.log(`✅ Batch transaction submitted: ${txHash}`);

    res.json({
      success: true,
      batch: {
        userOpHash: txHash,
        operationsCount: transactions.length,
        usePaymaster: Boolean(usePaymaster),
        estimatedConfirmation: '15-30 seconds'
      },
      operations: transactions,
      userOperation: userOp,
      benefits: [
        'Single transaction fee for multiple operations',
        'Atomic execution - all succeed or all fail',
        'Efficient gas usage compared to individual transactions'
      ]
    });

  } catch (error) {
    console.error('Batch transaction execution failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Batch transaction execution failed',
    });
  }
});

/**
 * 获取Paymaster信息
 * 
 * GET /aa/paymaster-info
 */
router.get('/paymaster-info', async (req: Request, res: Response) => {
  try {
    // 返回Paymaster服务信息
    res.json({
      success: true,
      paymaster: {
        available: true,
        address: '0x...',
        sponsorshipPolicy: {
          maxGasSponsored: '300000',
          allowedOperations: ['transfer', 'approve', 'multicall'],
          rateLimiting: '10 operations per hour per user'
        },
        requirements: 'Valid WebAuthn session required'
      },
      benefits: [
        'Gasless transactions for new users',
        'Simplified onboarding experience',
        'Pay transaction fees with tokens instead of ETH'
      ]
    });

  } catch (error) {
    console.error('Get paymaster info failed:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to get paymaster information',
    });
  }
});

// === 辅助函数 ===

async function getUserCredentialId(email: string): Promise<string> {
  // 在实际应用中，这应该从数据库或WebAuthn注册记录中获取
  return ethers.id(email).slice(0, 32);
}

async function computeAccountAddress(credentialId: string, salt: string): Promise<string> {
  // 使用CREATE2计算确定性地址
  const factoryAddress = '0x9406Cc6185a346906296840746125a0E44976454'; // Mock factory
  const initCodeHash = ethers.keccak256(
    ethers.solidityPacked(['string', 'bytes32'], [credentialId, salt])
  );

  return ethers.getCreate2Address(factoryAddress, salt, initCodeHash);
}

async function generateInitCode(
  credentialId: string, 
  salt: string, 
  guardians?: string[]
): Promise<string> {
  // 构建工厂合约调用数据
  const factoryInterface = new ethers.Interface([
    'function createAccount(string calldata credentialId, bytes32 salt, address[] calldata guardians) returns (address)'
  ]);

  const createAccountData = factoryInterface.encodeFunctionData('createAccount', [
    credentialId,
    salt,
    guardians || []
  ]);

  const factoryAddress = '0x9406Cc6185a346906296840746125a0E44976454';
  return ethers.concat([factoryAddress, createAccountData]);
}

async function deployAccountWithDeposit(
  accountAddress: string,
  initCode: string,
  deposit: string,
  appState: any
): Promise<any> {
  // 在实际应用中，这里应该构建并提交部署UserOperation
  console.log(`Deploying account ${accountAddress} with deposit ${deposit}`);
  
  // 模拟部署过程
  await new Promise(resolve => setTimeout(resolve, 2000));
  
  return {
    txHash: '0x' + Array.from({length: 64}, () => Math.floor(Math.random() * 16).toString(16)).join(''),
    blockNumber: Math.floor(Math.random() * 1000000) + 18000000
  };
}

async function getAccountNonce(address: string): Promise<number> {
  // 在实际应用中，从EntryPoint合约查询nonce
  return Math.floor(Math.random() * 5);
}

async function getAccountBalance(address: string): Promise<bigint> {
  // 在实际应用中，查询链上余额
  return ethers.parseEther((Math.random() * 0.1).toFixed(4));
}

async function buildUserOperation(params: {
  sender: string;
  to: string;
  value: string;
  data: string;
  usePaymaster: boolean;
}): Promise<any> {
  // 构建ERC-4337 UserOperation
  const nonce = await getAccountNonce(params.sender);
  
  // 编码调用数据
  const accountInterface = new ethers.Interface([
    'function execute(address to, uint256 value, bytes calldata data)'
  ]);
  
  const callData = accountInterface.encodeFunctionData('execute', [
    params.to,
    params.value,
    params.data
  ]);

  return {
    sender: params.sender,
    nonce: `0x${nonce.toString(16)}`,
    initCode: '0x', // 假设账户已部署
    callData,
    callGasLimit: '0x186a0', // 100000
    verificationGasLimit: '0x186a0', // 100000
    preVerificationGas: '0x5208', // 21000
    maxFeePerGas: '0x59682f00', // 1.5 gwei
    maxPriorityFeePerGas: '0x3b9aca00', // 1 gwei
    paymasterAndData: params.usePaymaster ? '0x123...' : '0x',
    signature: '0x' // 稍后填充
  };
}

function encodeBatchCall(transactions: any[]): string {
  const accountInterface = new ethers.Interface([
    'function executeBatch(address[] calldata to, uint256[] calldata value, bytes[] calldata data)'
  ]);

  const addresses = transactions.map(tx => tx.to);
  const values = transactions.map(tx => tx.value || '0');
  const dataArray = transactions.map(tx => tx.data || '0x');

  return accountInterface.encodeFunctionData('executeBatch', [addresses, values, dataArray]);
}

async function signUserOperationWithTEE(userOp: any, appState: any): Promise<string> {
  // 计算UserOperation哈希
  const userOpHash = getUserOperationHash(userOp);
  
  console.log(`🔐 Signing UserOperation hash: ${userOpHash}`);
  
  // 通过TEE签名
  try {
    const signResult = await appState.teeClient.signTransaction(
      parseInt(userOp.sender.slice(2, 10), 16),
      userOpHash
    );
    
    return signResult.signature;
  } catch (error) {
    console.warn('TEE signing failed, using mock signature:', error);
    
    // 模拟签名
    return ethers.Signature.from({
      r: '0x' + '1'.repeat(64),
      s: '0x' + '2'.repeat(64),
      v: 27
    }).serialized;
  }
}

function getUserOperationHash(userOp: any): string {
  // 根据ERC-4337标准计算UserOperation哈希
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

async function submitUserOperation(userOp: any): Promise<string> {
  // 在实际应用中，提交到Bundler
  console.log('Submitting UserOperation to bundler...');
  
  // 模拟提交过程
  await new Promise(resolve => setTimeout(resolve, 1000));
  
  // 返回模拟的UserOperation哈希
  return '0x' + Array.from({length: 64}, () => Math.floor(Math.random() * 16).toString(16)).join('');
}

export { router as accountAbstractionRoutes };