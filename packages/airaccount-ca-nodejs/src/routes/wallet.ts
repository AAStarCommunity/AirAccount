/**
 * 钱包路由 - 钱包操作
 * 
 * 重要：钱包访问需要用户提供恢复信息
 */

import { Router, Request, Response } from 'express';
import { z } from 'zod';
import type { AppState } from '../index.js';

const router = Router();

const CreateWalletSchema = z.object({
  sessionId: z.string(),
  email: z.string().email(),
  passkeyCredentialId: z.string(),
});

const WalletActionSchema = z.object({
  sessionId: z.string(),
  walletId: z.number().int().positive(),
});

const TransferSchema = z.object({
  sessionId: z.string(),
  walletId: z.number().int().positive(),
  toAddress: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  amount: z.string(),
  gasPrice: z.string().optional(),
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
 * 创建钱包
 * 需要用户已通过WebAuthn认证
 */
router.post('/create', requireAuth, async (req: Request, res: Response) => {
  try {
    const { sessionId, email, passkeyCredentialId } = CreateWalletSchema.parse(req.body);
    const appState = (req as any).appState as AppState;
    const session = (req as any).session;

    console.log(`🏦 Creating wallet for ${email}`);

    // 创建TEE钱包
    const dummyPublicKey = Buffer.from('webauthn_managed_key');
    const walletResult = await appState.teeClient.createAccountWithPasskey(
      email,
      passkeyCredentialId,
      dummyPublicKey
    );

    // 存储钱包会话信息
    await appState.database.storeWalletSession(sessionId, {
      walletId: walletResult.walletId,
      ethereumAddress: walletResult.ethereumAddress,
      teeDeviceId: walletResult.teeDeviceId,
    });

    console.log(`✅ Wallet created: ID=${walletResult.walletId}, Address=${walletResult.ethereumAddress}`);

    res.json({
      success: true,
      wallet: walletResult,
      userResponsibility: {
        recoveryInfo: {
          email: email,
          passkeyCredentialId: passkeyCredentialId,
          walletId: walletResult.walletId,
          ethereumAddress: walletResult.ethereumAddress,
        },
        message: "请保存恢复信息到安全位置，用于节点不可用时的钱包恢复"
      }
    });

  } catch (error) {
    console.error('Wallet creation failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Wallet creation failed',
    });
  }
});

/**
 * 查询余额
 */
router.post('/balance', requireAuth, async (req: Request, res: Response) => {
  try {
    const { sessionId, walletId } = WalletActionSchema.parse(req.body);
    const appState = (req as any).appState as AppState;

    console.log(`💰 Getting balance for wallet: ${walletId}`);

    // 获取钱包信息
    const walletInfo = await appState.teeClient.getWalletInfo(walletId);
    const address = await appState.teeClient.deriveAddress(walletId);

    // 模拟余额查询（实际应用中需要查询区块链）
    const balanceWei = "1000000000000000000"; // 1 ETH
    const balanceEth = "1.0";

    res.json({
      success: true,
      wallet: {
        walletId,
        ethereumAddress: address,
        balance: {
          wei: balanceWei,
          eth: balanceEth,
        },
        info: walletInfo,
      }
    });

  } catch (error) {
    console.error('Balance query failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Balance query failed',
    });
  }
});

/**
 * 转账
 */
router.post('/transfer', requireAuth, async (req: Request, res: Response) => {
  try {
    const { sessionId, walletId, toAddress, amount, gasPrice } = TransferSchema.parse(req.body);
    const appState = (req as any).appState as AppState;

    console.log(`💸 Processing transfer: wallet=${walletId}, to=${toAddress}, amount=${amount}`);

    // 构建交易数据
    const transactionData = JSON.stringify({
      to: toAddress,
      amount: amount,
      gasPrice: gasPrice || "20000000000",
      timestamp: Date.now(),
    });

    // TEE签名
    const result = await appState.teeClient.signTransaction(walletId, transactionData);

    console.log(`✅ Transaction signed: hash=${result.transactionHash}`);

    res.json({
      success: true,
      transaction: result,
      notice: "交易已签名，请自行广播到区块链网络"
    });

  } catch (error) {
    console.error('Transfer failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Transfer failed',
    });
  }
});

/**
 * 列出钱包
 */
router.get('/list', requireAuth, async (req: Request, res: Response) => {
  try {
    const appState = (req as any).appState as AppState;
    const session = (req as any).session;

    console.log(`📋 Listing wallets for user: ${session.email}`);

    const wallets = await appState.teeClient.listWallets();

    res.json({
      success: true,
      wallets,
      userEmail: session.email,
      notice: "这些是当前节点中的钱包信息，用户应保留自己的恢复凭证"
    });

  } catch (error) {
    console.error('List wallets failed:', error);
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'List wallets failed',
    });
  }
});

export { router as walletRoutes };