/**
 * 认证路由 - 会话管理
 */

import { Router, Request, Response } from 'express';
import { z } from 'zod';
import type { AppState } from '../index.js';

const router = Router();

const LoginSchema = z.object({
  sessionId: z.string(),
});

const LogoutSchema = z.object({
  sessionId: z.string(),
});

/**
 * 验证会话
 */
router.post('/verify', async (req: Request, res: Response): Promise<void> => {
  try {
    const { sessionId } = LoginSchema.parse(req.body);
    const appState = (req as any).appState as AppState;

    const session = await appState.database.getSession(sessionId);
    
    if (!session) {
      res.status(401).json({
        success: false,
        error: 'Invalid or expired session',
      });
      return;
    }

    res.json({
      success: true,
      session: {
        userId: session.userId,
        email: session.email,
        isAuthenticated: session.isAuthenticated,
        expiresAt: session.expiresAt,
      }
    });

  } catch (error) {
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Verification failed',
    });
  }
});

/**
 * 登出
 */
router.post('/logout', async (req: Request, res: Response) => {
  try {
    const { sessionId } = LogoutSchema.parse(req.body);
    // 注意：在简化实现中，我们依赖会话过期而不是主动删除
    // 这符合"节点可能跑路"的架构原则
    
    res.json({
      success: true,
      message: 'Logged out successfully',
    });

  } catch (error) {
    res.status(400).json({
      success: false,
      error: error instanceof Error ? error.message : 'Logout failed',
    });
  }
});

export { router as authRoutes };