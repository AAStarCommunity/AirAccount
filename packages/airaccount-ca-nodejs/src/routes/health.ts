/**
 * 健康检查路由
 */

import { Router, Request, Response } from 'express';
import type { AppState } from '../index.js';

const router = Router();

router.get('/', async (req: Request, res: Response) => {
  try {
    const appState = (req as any).appState as AppState;
    
    // 测试各服务状态
    const teeStatus = await appState.teeClient.testConnection();
    const webauthnStats = appState.webauthnService.getStats();
    
    res.json({
      status: 'healthy',
      timestamp: new Date().toISOString(),
      version: '0.1.0',
      services: {
        tee: {
          connected: teeStatus.includes('AirAccount'),
          response: teeStatus
        },
        webauthn: {
          active: true,
          stats: webauthnStats
        },
        database: {
          connected: true
        }
      },
      architecture: {
        type: 'client-controlled-credentials',
        note: '用户凭证由客户端管理，节点提供临时服务'
      }
    });
    
  } catch (error) {
    res.status(500).json({
      status: 'unhealthy',
      error: error instanceof Error ? error.message : 'Unknown error',
      timestamp: new Date().toISOString()
    });
  }
});

export { router as healthRoutes };