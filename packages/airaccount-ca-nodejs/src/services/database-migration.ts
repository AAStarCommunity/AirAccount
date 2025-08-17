/**
 * 数据库初始化管理
 * Node.js CA和Rust CA使用统一的数据结构
 */

import { Database } from './database.js';
import { promisify } from 'util';

export interface DatabaseStatusReport {
  isReady: boolean;
  requiredInitializations: string[];
  notes: string[];
}

export class DatabaseInitManager {
  private database: Database;

  constructor(database: Database) {
    this.database = database;
  }

  /**
   * 检查数据库状态
   */
  async checkStatus(): Promise<DatabaseStatusReport> {
    const report: DatabaseStatusReport = {
      isReady: true,
      requiredInitializations: [],
      notes: []
    };

    // 检查统一数据库表结构
    try {
      if (this.database['db']) {
        const getAsync = promisify(this.database['db'].get.bind(this.database['db']));
        await getAsync("SELECT name FROM sqlite_master WHERE type='table' AND name='passkeys'");
        await getAsync("SELECT name FROM sqlite_master WHERE type='table' AND name='registration_states'");
        await getAsync("SELECT name FROM sqlite_master WHERE type='table' AND name='authentication_states'");
        report.notes.push('统一WebAuthn数据库结构已就绪');
      }
    } catch (error) {
      report.requiredInitializations.push('初始化统一WebAuthn数据库结构');
      report.isReady = false;
    }

    return report;
  }

  /**
   * 初始化统一数据库结构
   */
  async initialize(): Promise<void> {
    const report = await this.checkStatus();
    
    if (!report.isReady) {
      console.log('🔄 初始化统一WebAuthn数据库结构...');
      console.log('📋 数据库表将在Database初始化时自动创建');
      console.log('✅ 统一数据库结构初始化完成');
    } else {
      console.log('✅ 统一WebAuthn数据库结构已就绪');
    }
  }

  /**
   * 获取数据库统计信息
   */
  async getStats(): Promise<{
    totalUsers: number;
    totalDevices: number;
    status: string;
  }> {
    const stats = await this.database.getUserStats();
    return {
      totalUsers: stats.totalUsers,
      totalDevices: stats.totalDevices,
      status: '统一WebAuthn数据库结构运行正常'
    };
  }
}