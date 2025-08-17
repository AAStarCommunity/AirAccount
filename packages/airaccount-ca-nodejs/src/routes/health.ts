/**
 * 健康检查路由
 */

import { Router, Request, Response } from 'express';
import type { AppState } from '../index.js';

const router = Router();

router.get('/', async (req: Request, res: Response) => {
  try {
    const appState = (req as any).appState as AppState;
    
    // 测试各服务状态 - 为TEE连接添加超时
    let teeStatus = 'TEE connection timeout or initializing';
    let teeConnected = false;
    
    try {
      // 使用Promise.race添加5秒超时
      const teePromise = appState.teeClient.testConnection();
      const timeoutPromise = new Promise<string>((_, reject) => 
        setTimeout(() => reject(new Error('TEE connection timeout')), 5000)
      );
      
      teeStatus = await Promise.race([teePromise, timeoutPromise]);
      teeConnected = teeStatus.includes('AirAccount');
    } catch (error) {
      // TEE连接超时或失败，继续返回其他服务状态
      teeStatus = `TEE unavailable: ${error instanceof Error ? error.message : 'Unknown error'}`;
      teeConnected = false;
    }
    
    const webauthnStats = await appState.webauthnService.getStats();
    
    res.json({
      status: 'healthy',
      timestamp: new Date().toISOString(),
      version: '0.1.0',
      services: {
        tee: {
          connected: teeConnected,
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