/**
 * AirAccount CA Service - Node.js Implementation
 * 集成Simple WebAuthn的现代化CA服务
 * 
 * 架构：浏览器WebAuthn → Simple WebAuthn → CA HTTP API → TEE TA
 */

import express from 'express';
import cors from 'cors';
import helmet from 'helmet';
import dotenv from 'dotenv';
import { createLogger, format, transports } from 'winston';

import { authRoutes } from './routes/auth.js';
import { walletRoutes } from './routes/wallet.js';
import { webauthnRoutes } from './routes/webauthn.js';
import { healthRoutes } from './routes/health.js';
import { accountAbstractionRoutes } from './routes/account-abstraction.js';
import { TEEClient } from './services/tee-client.js';
import { Database } from './services/database.js';
import { WebAuthnService } from './services/webauthn.js';

// 加载环境变量
dotenv.config();

// 创建日志器
const logger = createLogger({
  level: process.env.LOG_LEVEL || 'info',
  format: format.combine(
    format.timestamp(),
    format.errors({ stack: true }),
    format.json()
  ),
  transports: [
    new transports.Console({
      format: format.combine(
        format.colorize(),
        format.simple()
      )
    }),
    new transports.File({ filename: 'logs/ca-nodejs-error.log', level: 'error' }),
    new transports.File({ filename: 'logs/ca-nodejs-combined.log' })
  ]
});

// 应用状态
interface AppState {
  teeClient: TEEClient;
  database: Database;
  webauthnService: WebAuthnService;
}

async function createApp(): Promise<express.Application> {
  const app = express();

  // 中间件
  app.use(helmet());
  app.use(cors({
    origin: process.env.CORS_ORIGIN || 'http://localhost:3000',
    credentials: true
  }));
  app.use(express.json({ limit: '10mb' }));
  app.use(express.urlencoded({ extended: true }));

  // 请求日志
  app.use((req, res, next) => {
    logger.info(`${req.method} ${req.path}`, {
      ip: req.ip,
      userAgent: req.get('User-Agent')
    });
    next();
  });

  // 初始化服务
  logger.info('🔧 Initializing AirAccount CA Node.js services...');

  const database = new Database();
  await database.initialize();
  logger.info('✅ Database initialized');

  const teeClient = new TEEClient();
  
  // Initialize TEE Client asynchronously (non-blocking)
  teeClient.initialize()
    .then(() => {
      logger.info('✅ TEE client initialized successfully');
    })
    .catch((error) => {
      logger.error('❌ TEE client initialization failed:', error);
      logger.warn('⚠️ Running in mock mode without real TEE');
    });

  const isTestMode = process.env.NODE_ENV === 'test' || process.env.WEBAUTHN_TEST_MODE === 'true';
  const webauthnService = new WebAuthnService({
    rpName: process.env.RP_NAME || 'AirAccount',
    rpID: process.env.RP_ID || 'localhost',
    origin: process.env.WEBAUTHN_ORIGIN || 'http://localhost:3002'
  }, database, isTestMode);
  logger.info(`✅ WebAuthn service initialized${isTestMode ? ' (Test Mode)' : ''}`);

  // 应用状态
  const appState: AppState = {
    teeClient,
    database,
    webauthnService
  };

  // 将状态添加到请求对象
  app.use((req, res, next) => {
    (req as any).appState = appState;
    next();
  });

  // 路由
  app.use('/health', healthRoutes);
  app.use('/api/webauthn', webauthnRoutes);
  app.use('/api/auth', authRoutes);
  app.use('/api/wallet', walletRoutes);
  app.use('/api/aa', accountAbstractionRoutes);

  // 404处理
  app.use('*', (req, res) => {
    res.status(404).json({
      success: false,
      error: 'Endpoint not found',
      path: req.originalUrl
    });
  });

  // 错误处理
  app.use((err: any, req: express.Request, res: express.Response, next: express.NextFunction) => {
    logger.error('Unhandled error:', err);
    
    res.status(err.status || 500).json({
      success: false,
      error: err.message || 'Internal server error',
      ...(process.env.NODE_ENV === 'development' && { stack: err.stack })
    });
  });

  return app;
}

async function startServer() {
  try {
    const app = await createApp();
    const port = process.env.PORT ? parseInt(process.env.PORT) : 3002;

    app.listen(port, '0.0.0.0', () => {
      logger.info(`🌐 AirAccount CA Node.js server listening on http://0.0.0.0:${port}`);
      logger.info('📚 API Endpoints:');
      logger.info('  GET  /health - 健康检查');
      logger.info('  POST /api/webauthn/register/begin - 开始WebAuthn注册');
      logger.info('  POST /api/webauthn/register/finish - 完成WebAuthn注册');
      logger.info('  POST /api/webauthn/authenticate/begin - 开始WebAuthn认证');
      logger.info('  POST /api/webauthn/authenticate/finish - 完成WebAuthn认证');
      logger.info('  POST /api/auth/login - 用户登录');
      logger.info('  POST /api/wallet/create - 创建钱包');
      logger.info('  POST /api/wallet/balance - 查询余额');
      logger.info('  POST /api/wallet/transfer - 转账');
      logger.info('  GET  /api/wallet/list - 列出钱包');
      logger.info('  POST /api/aa/create-account - 创建抽象账户');
      logger.info('  POST /api/aa/account-info - 获取账户信息');
      logger.info('  POST /api/aa/execute-transaction - 执行交易');
      logger.info('  POST /api/aa/execute-batch - 批量执行交易');
      logger.info('  GET  /api/aa/paymaster-info - Paymaster信息');
    });

  } catch (error) {
    logger.error('Failed to start server:', error);
    process.exit(1);
  }
}

// 优雅关闭
process.on('SIGTERM', () => {
  logger.info('SIGTERM received, shutting down gracefully');
  process.exit(0);
});

process.on('SIGINT', () => {
  logger.info('SIGINT received, shutting down gracefully');
  process.exit(0);
});

// 启动服务器
startServer();

export type { AppState };