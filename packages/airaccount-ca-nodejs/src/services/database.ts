/**
 * 数据库服务 - 用于临时会话和非关键数据
 * 
 * 重要架构原则：
 * - 节点可能跑路，用户凭证（Passkey + Email）必须由用户自己存储
 * - 此数据库只存储临时会话数据和非关键信息
 * - 用户的Passkey凭证应存储在客户端（浏览器、移动设备）
 */

import sqlite3 from 'sqlite3';
import { promisify } from 'util';
import crypto from 'crypto';

// 类型定义
type DbRunAsync = (sql: string, params?: any[]) => Promise<any>;
type DbGetAsync = (sql: string, params?: any[]) => Promise<any>;

export interface SessionData {
  sessionId: string;
  userId: string;
  email: string;
  isAuthenticated: boolean;
  createdAt: number;
  expiresAt: number;
  lastActivity: number;
}

export interface WalletSession {
  sessionId: string;
  walletId: number;
  ethereumAddress: string;
  teeDeviceId: string;
  createdAt: number;
}

export interface ChallengeRecord {
  challenge: string;
  userId: string;
  challengeType: 'registration' | 'authentication';
  createdAt: number;
  expiresAt: number;
  used: boolean;
}

export interface DbUserAccount {
  userId: string;
  username: string;
  displayName: string;
  createdAt: number;
  updatedAt: number;
}

export interface AuthenticatorDevice {
  id?: number;
  userId: string;
  credentialId: Buffer;
  credentialPublicKey: Buffer;
  counter: number;
  transports: string[]; // JSON array
  createdAt: number;
  updatedAt: number;
}

export class Database {
  private db: sqlite3.Database | null = null;
  private dbPath: string;

  constructor(dbPath: string = ':memory:') {
    // 使用内存数据库，强调临时性
    // 生产环境可以使用文件数据库，但要明确这些数据不是用户资产的关键部分
    this.dbPath = dbPath;
  }

  async initialize(): Promise<void> {
    this.db = new sqlite3.Database(this.dbPath);
    
    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;

    // 创建会话表 - 仅用于临时会话管理
    await runAsync(`
      CREATE TABLE IF NOT EXISTS sessions (
        session_id TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        email TEXT NOT NULL,
        is_authenticated BOOLEAN DEFAULT FALSE,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        last_activity INTEGER NOT NULL
      )
    `);

    // 创建钱包会话表 - 临时存储当前会话的钱包信息
    await runAsync(`
      CREATE TABLE IF NOT EXISTS wallet_sessions (
        session_id TEXT PRIMARY KEY,
        wallet_id INTEGER NOT NULL,
        ethereum_address TEXT NOT NULL,
        tee_device_id TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        FOREIGN KEY (session_id) REFERENCES sessions (session_id) ON DELETE CASCADE
      )
    `);

    // 创建挑战记录表 - 用于WebAuthn挑战防重放
    await runAsync(`
      CREATE TABLE IF NOT EXISTS challenges (
        challenge TEXT PRIMARY KEY,
        user_id TEXT NOT NULL,
        challenge_type TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        used BOOLEAN DEFAULT FALSE
      )
    `);

    // 创建用户账户表 - 存储WebAuthn用户信息
    await runAsync(`
      CREATE TABLE IF NOT EXISTS user_accounts (
        user_id TEXT PRIMARY KEY,
        username TEXT NOT NULL,
        display_name TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      )
    `);

    // 创建认证设备表 - 存储用户的Passkey设备
    await runAsync(`
      CREATE TABLE IF NOT EXISTS authenticator_devices (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id TEXT NOT NULL,
        credential_id BLOB NOT NULL UNIQUE,
        credential_public_key BLOB NOT NULL,
        counter INTEGER NOT NULL DEFAULT 0,
        transports TEXT, -- JSON array of transport methods
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        FOREIGN KEY (user_id) REFERENCES user_accounts (user_id) ON DELETE CASCADE
      )
    `);

    // 创建索引
    await runAsync('CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions (user_id)');
    await runAsync('CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions (expires_at)');
    await runAsync('CREATE INDEX IF NOT EXISTS idx_challenges_expires_at ON challenges (expires_at)');
    await runAsync('CREATE INDEX IF NOT EXISTS idx_authenticator_devices_user_id ON authenticator_devices (user_id)');
    await runAsync('CREATE INDEX IF NOT EXISTS idx_authenticator_devices_credential_id ON authenticator_devices (credential_id)');

    // 启动清理定时器
    this.startCleanupTimer();
  }

  /**
   * 创建会话
   * 注意：这只是临时会话，用户真正的凭证应由客户端管理
   */
  async createSession(userId: string, email: string, ttlSeconds: number = 3600): Promise<string> {
    if (!this.db) throw new Error('Database not initialized');

    const sessionId = this.generateSessionId();
    const now = Date.now();
    const expiresAt = now + (ttlSeconds * 1000);

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    
    await runAsync(`
      INSERT INTO sessions (session_id, user_id, email, is_authenticated, created_at, expires_at, last_activity)
      VALUES (?, ?, ?, FALSE, ?, ?, ?)
    `, [sessionId, userId, email, now, expiresAt, now]);

    return sessionId;
  }

  /**
   * 验证会话
   */
  async getSession(sessionId: string): Promise<SessionData | null> {
    if (!this.db) throw new Error('Database not initialized');

    const getAsync = promisify(this.db.get.bind(this.db)) as DbGetAsync;
    
    const row = await getAsync(`
      SELECT * FROM sessions 
      WHERE session_id = ? AND expires_at > ?
    `, [sessionId, Date.now()]) as any;

    if (!row) return null;

    // 更新最后活动时间
    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    await runAsync(`
      UPDATE sessions SET last_activity = ? WHERE session_id = ?
    `, [Date.now(), sessionId]);

    return {
      sessionId: row.session_id,
      userId: row.user_id,
      email: row.email,
      isAuthenticated: row.is_authenticated === 1,
      createdAt: row.created_at,
      expiresAt: row.expires_at,
      lastActivity: row.last_activity,
    };
  }

  /**
   * 标记会话为已认证
   */
  async authenticateSession(sessionId: string): Promise<void> {
    if (!this.db) throw new Error('Database not initialized');

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    
    await runAsync(`
      UPDATE sessions 
      SET is_authenticated = TRUE, last_activity = ?
      WHERE session_id = ?
    `, [Date.now(), sessionId]);
  }

  /**
   * 存储挑战（用于防重放攻击）
   */
  async storeChallenge(challenge: string, userId: string, type: 'registration' | 'authentication'): Promise<void> {
    if (!this.db) throw new Error('Database not initialized');

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    const now = Date.now();
    const expiresAt = now + (5 * 60 * 1000); // 5分钟过期

    // 使用 INSERT OR REPLACE 处理重复challenge的情况
    await runAsync(`
      INSERT OR REPLACE INTO challenges (challenge, user_id, challenge_type, created_at, expires_at, used)
      VALUES (?, ?, ?, ?, ?, FALSE)
    `, [challenge, userId, type, now, expiresAt]);
  }

  /**
   * 验证并标记挑战为已使用
   */
  async verifyAndUseChallenge(challenge: string, expectedUserId?: string): Promise<boolean> {
    if (!this.db) throw new Error('Database not initialized');

    const getAsync = promisify(this.db.get.bind(this.db)) as DbGetAsync;
    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;

    // 检查挑战是否存在且未过期
    const row = await getAsync(`
      SELECT * FROM challenges 
      WHERE challenge = ? AND expires_at > ? AND used = FALSE
    `, [challenge, Date.now()]) as any;

    if (!row) return false;

    // 如果指定了用户ID，验证是否匹配
    if (expectedUserId && row.user_id !== expectedUserId) {
      return false;
    }

    // 标记为已使用
    await runAsync(`
      UPDATE challenges SET used = TRUE WHERE challenge = ?
    `, [challenge]);

    return true;
  }

  /**
   * 存储钱包会话信息（临时）
   */
  async storeWalletSession(sessionId: string, walletInfo: Omit<WalletSession, 'sessionId' | 'createdAt'>): Promise<void> {
    if (!this.db) throw new Error('Database not initialized');

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    
    await runAsync(`
      INSERT OR REPLACE INTO wallet_sessions 
      (session_id, wallet_id, ethereum_address, tee_device_id, created_at)
      VALUES (?, ?, ?, ?, ?)
    `, [sessionId, walletInfo.walletId, walletInfo.ethereumAddress, walletInfo.teeDeviceId, Date.now()]);
  }

  /**
   * 获取钱包会话信息
   */
  async getWalletSession(sessionId: string): Promise<WalletSession | null> {
    if (!this.db) throw new Error('Database not initialized');

    const getAsync = promisify(this.db.get.bind(this.db)) as DbGetAsync;
    
    const row = await getAsync(`
      SELECT * FROM wallet_sessions WHERE session_id = ?
    `, [sessionId]) as any;

    if (!row) return null;

    return {
      sessionId: row.session_id,
      walletId: row.wallet_id,
      ethereumAddress: row.ethereum_address,
      teeDeviceId: row.tee_device_id,
      createdAt: row.created_at,
    };
  }

  /**
   * 清理过期数据
   */
  async cleanup(): Promise<void> {
    if (!this.db) return;

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    const now = Date.now();

    // 清理过期会话
    await runAsync('DELETE FROM sessions WHERE expires_at < ?', [now]);
    
    // 清理过期挑战
    await runAsync('DELETE FROM challenges WHERE expires_at < ?', [now]);
  }

  /**
   * 关闭数据库连接
   */
  async close(): Promise<void> {
    if (this.db) {
      const closeAsync = promisify(this.db.close.bind(this.db));
      await closeAsync();
      this.db = null;
    }
  }

  // 私有方法

  private generateSessionId(): string {
    return crypto.randomBytes(32).toString('hex');
  }

  private startCleanupTimer(): void {
    // 每10分钟清理一次过期数据
    setInterval(async () => {
      try {
        await this.cleanup();
      } catch (error) {
        console.error('Database cleanup failed:', error);
      }
    }, 10 * 60 * 1000);
  }

  // ========== 用户账户管理 ==========

  /**
   * 创建或更新用户账户
   */
  async createOrUpdateUser(userId: string, username: string, displayName: string): Promise<void> {
    if (!this.db) throw new Error('Database not initialized');

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    const now = Date.now();

    await runAsync(`
      INSERT OR REPLACE INTO user_accounts (user_id, username, display_name, created_at, updated_at)
      VALUES (?, ?, ?, 
        COALESCE((SELECT created_at FROM user_accounts WHERE user_id = ?), ?),
        ?
      )
    `, [userId, username, displayName, userId, now, now]);
  }

  /**
   * 获取用户账户
   */
  async getUserAccount(userId: string): Promise<DbUserAccount | null> {
    if (!this.db) throw new Error('Database not initialized');

    const getAsync = promisify(this.db.get.bind(this.db)) as DbGetAsync;
    
    const row = await getAsync(`
      SELECT * FROM user_accounts WHERE user_id = ?
    `, [userId]) as any;

    if (!row) return null;

    return {
      userId: row.user_id,
      username: row.username,
      displayName: row.display_name,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    };
  }

  /**
   * 添加认证设备
   */
  async addAuthenticatorDevice(device: Omit<AuthenticatorDevice, 'id' | 'createdAt' | 'updatedAt'>): Promise<number> {
    if (!this.db) throw new Error('Database not initialized');

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    const now = Date.now();

    const result = await runAsync(`
      INSERT INTO authenticator_devices 
      (user_id, credential_id, credential_public_key, counter, transports, created_at, updated_at)
      VALUES (?, ?, ?, ?, ?, ?, ?)
    `, [
      device.userId,
      device.credentialId,
      device.credentialPublicKey,
      device.counter,
      JSON.stringify(device.transports),
      now,
      now
    ]);

    return result?.lastID || Date.now(); // 兜底返回时间戳作为ID
  }

  /**
   * 获取用户的所有认证设备
   */
  async getUserDevices(userId: string): Promise<AuthenticatorDevice[]> {
    if (!this.db) throw new Error('Database not initialized');

    const getAsync = promisify(this.db.all.bind(this.db)) as (sql: string, params?: any[]) => Promise<any[]>;
    
    const rows = await getAsync(`
      SELECT * FROM authenticator_devices WHERE user_id = ? ORDER BY created_at DESC
    `, [userId]);

    return rows.map(row => ({
      id: row.id,
      userId: row.user_id,
      credentialId: row.credential_id,
      credentialPublicKey: row.credential_public_key,
      counter: row.counter,
      transports: JSON.parse(row.transports || '[]'),
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    }));
  }

  /**
   * 通过凭证ID查找设备
   */
  async getDeviceByCredentialId(credentialId: Buffer): Promise<AuthenticatorDevice | null> {
    if (!this.db) throw new Error('Database not initialized');

    const getAsync = promisify(this.db.get.bind(this.db)) as DbGetAsync;
    
    const row = await getAsync(`
      SELECT * FROM authenticator_devices WHERE credential_id = ?
    `, [credentialId]) as any;

    if (!row) return null;

    return {
      id: row.id,
      userId: row.user_id,
      credentialId: row.credential_id,
      credentialPublicKey: row.credential_public_key,
      counter: row.counter,
      transports: JSON.parse(row.transports || '[]'),
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    };
  }

  /**
   * 更新设备计数器
   */
  async updateDeviceCounter(credentialId: Buffer, newCounter: number): Promise<void> {
    if (!this.db) throw new Error('Database not initialized');

    const runAsync = promisify(this.db.run.bind(this.db)) as DbRunAsync;
    
    await runAsync(`
      UPDATE authenticator_devices 
      SET counter = ?, updated_at = ? 
      WHERE credential_id = ?
    `, [newCounter, Date.now(), credentialId]);
  }

  /**
   * 获取用户账户和设备统计信息
   */
  async getUserStats(): Promise<{ totalUsers: number; totalDevices: number }> {
    if (!this.db) throw new Error('Database not initialized');

    const getAsync = promisify(this.db.get.bind(this.db)) as DbGetAsync;
    
    const userCount = await getAsync('SELECT COUNT(*) as count FROM user_accounts') as any;
    const deviceCount = await getAsync('SELECT COUNT(*) as count FROM authenticator_devices') as any;

    return {
      totalUsers: userCount.count,
      totalDevices: deviceCount.count,
    };
  }
}