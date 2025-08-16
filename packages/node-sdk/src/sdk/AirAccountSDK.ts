// AirAccount SDK主类
// 提供完整的AirAccount功能接口

import { EventEmitter } from 'eventemitter3';
import { ethers } from 'ethers';
import { 
  AirAccountConfig, 
  SDKEvents,
  DeviceInfo,
  WalletInfo,
  ErrorCode,
  AirAccountError,
  VersionInfo
} from '../types';
import { WalletManager } from '../wallet/WalletManager';
import { TEEDevice } from '../device/TEEDevice';
import { TransactionSigner } from '../crypto/TransactionSigner';
import { DEFAULT_CONFIG, VERSION_INFO } from '../constants';
import { createError } from '../utils';

/**
 * AirAccount SDK主类
 * 
 * @example
 * ```typescript
 * const sdk = new AirAccountSDK({
 *   network: 'mainnet',
 *   teeConfig: {
 *     connectionType: 'bluetooth'
 *   }
 * });
 * 
 * await sdk.initialize();
 * const devices = await sdk.discoverDevices();
 * await sdk.connectDevice(devices[0].id);
 * 
 * const wallet = await sdk.createWallet();
 * const signature = await sdk.signTransaction({
 *   to: '0x...',
 *   value: '1000000000000000000',
 *   chainId: 1
 * });
 * ```
 */
export class AirAccountSDK extends EventEmitter<SDKEvents> {
  private config: AirAccountConfig;
  private isInitialized: boolean = false;
  
  // 核心组件
  private walletManager: WalletManager | null = null;
  private teeDevice: TEEDevice | null = null;
  private transactionSigner: TransactionSigner | null = null;
  
  // 状态管理
  private currentDevice: DeviceInfo | null = null;
  private currentWallet: WalletInfo | null = null;

  constructor(config: Partial<AirAccountConfig> = {}) {
    super();
    
    // 合并默认配置
    this.config = {
      ...DEFAULT_CONFIG,
      ...config,
      teeConfig: {
        ...DEFAULT_CONFIG.teeConfig,
        ...config.teeConfig
      }
    };
  }

  // === 初始化方法 ===

  /**
   * 初始化SDK
   */
  async initialize(): Promise<void> {
    if (this.isInitialized) {
      return;
    }

    try {
      // 验证配置
      this.validateConfig();
      
      // 初始化核心组件
      this.walletManager = new WalletManager(this.config);
      this.teeDevice = new TEEDevice(this.config.teeConfig!);
      this.transactionSigner = new TransactionSigner(this.config);
      
      // 设置事件转发
      this.setupEventForwarding();
      
      // 初始化组件
      await this.walletManager.initialize();
      await this.teeDevice.initialize();
      
      this.isInitialized = true;
      this.emit('ready');
      
    } catch (error) {
      const sdkError = this.wrapError(error, 'initialize');
      this.emit('error', sdkError);
      throw sdkError;
    }
  }

  /**
   * 销毁SDK实例
   */
  async destroy(): Promise<void> {
    if (!this.isInitialized) {
      return;
    }

    try {
      // 断开设备连接
      if (this.currentDevice) {
        await this.disconnectDevice();
      }
      
      // 销毁组件
      if (this.teeDevice) {
        await this.teeDevice.destroy();
      }
      
      if (this.walletManager) {
        await this.walletManager.destroy();
      }
      
      // 清理状态
      this.walletManager = null;
      this.teeDevice = null;
      this.transactionSigner = null;
      this.currentDevice = null;
      this.currentWallet = null;
      
      this.isInitialized = false;
      this.removeAllListeners();
      
    } catch (error) {
      const sdkError = this.wrapError(error, 'destroy');
      this.emit('error', sdkError);
      throw sdkError;
    }
  }

  // === 设备管理方法 ===

  /**
   * 发现TEE设备
   */
  async discoverDevices(): Promise<DeviceInfo[]> {
    this.ensureInitialized();
    
    try {
      return await this.teeDevice!.discoverDevices();
    } catch (error) {
      throw this.wrapError(error, 'discoverDevices');
    }
  }

  /**
   * 连接TEE设备
   */
  async connectDevice(deviceId: string): Promise<void> {
    this.ensureInitialized();
    
    try {
      const device = await this.teeDevice!.connect(deviceId);
      this.currentDevice = device;
      this.emit('device:connected', device);
    } catch (error) {
      throw this.wrapError(error, 'connectDevice');
    }
  }

  /**
   * 断开设备连接
   */
  async disconnectDevice(): Promise<void> {
    this.ensureInitialized();
    
    if (!this.currentDevice) {
      return;
    }

    try {
      await this.teeDevice!.disconnect();
      const device = this.currentDevice;
      this.currentDevice = null;
      this.emit('device:disconnected', device);
    } catch (error) {
      throw this.wrapError(error, 'disconnectDevice');
    }
  }

  /**
   * 获取设备状态
   */
  getDeviceInfo(): DeviceInfo | null {
    return this.currentDevice;
  }

  /**
   * 检查设备连接状态
   */
  isDeviceConnected(): boolean {
    return this.currentDevice !== null && this.currentDevice.state === 'connected';
  }

  // === 钱包管理方法 ===

  /**
   * 创建新钱包
   */
  async createWallet(options?: { 
    strength?: 128 | 160 | 192 | 224 | 256;
    password?: string;
    name?: string;
  }): Promise<WalletInfo> {
    this.ensureInitialized();
    this.ensureDeviceConnected();
    
    try {
      const wallet = await this.walletManager!.createWallet(options);
      this.currentWallet = wallet;
      this.emit('wallet:created', wallet);
      return wallet;
    } catch (error) {
      throw this.wrapError(error, 'createWallet');
    }
  }

  /**
   * 导入钱包
   */
  async importWallet(options: {
    mnemonic: string;
    password?: string;
    name?: string;
    derivationPath?: string;
  }): Promise<WalletInfo> {
    this.ensureInitialized();
    this.ensureDeviceConnected();
    
    try {
      const wallet = await this.walletManager!.importWallet(options);
      this.currentWallet = wallet;
      return wallet;
    } catch (error) {
      throw this.wrapError(error, 'importWallet');
    }
  }

  /**
   * 获取钱包列表
   */
  async getWallets(): Promise<WalletInfo[]> {
    this.ensureInitialized();
    
    try {
      return await this.walletManager!.getWallets();
    } catch (error) {
      throw this.wrapError(error, 'getWallets');
    }
  }

  /**
   * 选择活动钱包
   */
  async selectWallet(address: string): Promise<WalletInfo> {
    this.ensureInitialized();
    
    try {
      const wallet = await this.walletManager!.getWallet(address);
      this.currentWallet = wallet;
      return wallet;
    } catch (error) {
      throw this.wrapError(error, 'selectWallet');
    }
  }

  /**
   * 获取当前钱包
   */
  getCurrentWallet(): WalletInfo | null {
    return this.currentWallet;
  }

  /**
   * 锁定钱包
   */
  async lockWallet(): Promise<void> {
    this.ensureInitialized();
    
    if (!this.currentWallet) {
      return;
    }

    try {
      await this.walletManager!.lockWallet(this.currentWallet.address);
      this.currentWallet.status = 'locked';
      this.emit('wallet:locked', this.currentWallet);
    } catch (error) {
      throw this.wrapError(error, 'lockWallet');
    }
  }

  /**
   * 解锁钱包
   */
  async unlockWallet(password?: string): Promise<void> {
    this.ensureInitialized();
    this.ensureDeviceConnected();
    
    if (!this.currentWallet) {
      throw createError(ErrorCode.WALLET_NOT_FOUND, '未选择钱包');
    }

    try {
      await this.walletManager!.unlockWallet(this.currentWallet.address, password);
      this.currentWallet.status = 'unlocked';
      this.emit('wallet:unlocked', this.currentWallet);
    } catch (error) {
      throw this.wrapError(error, 'unlockWallet');
    }
  }

  // === 资产管理方法 ===

  /**
   * 获取钱包余额
   */
  async getBalance(chainId: number, address?: string): Promise<any> {
    this.ensureInitialized();
    
    const walletAddress = address || this.currentWallet?.address;
    if (!walletAddress) {
      throw createError(ErrorCode.WALLET_NOT_FOUND, '未指定钱包地址');
    }

    try {
      return await this.walletManager!.getBalance(walletAddress, chainId);
    } catch (error) {
      throw this.wrapError(error, 'getBalance');
    }
  }

  /**
   * 获取交易历史
   */
  async getTransactionHistory(chainId: number, address?: string): Promise<any[]> {
    this.ensureInitialized();
    
    const walletAddress = address || this.currentWallet?.address;
    if (!walletAddress) {
      throw createError(ErrorCode.WALLET_NOT_FOUND, '未指定钱包地址');
    }

    try {
      return await this.walletManager!.getTransactionHistory(walletAddress, chainId);
    } catch (error) {
      throw this.wrapError(error, 'getTransactionHistory');
    }
  }

  // === 交易签名方法 ===

  /**
   * 签名交易
   */
  async signTransaction(transaction: any, options?: { broadcast?: boolean }): Promise<any> {
    this.ensureInitialized();
    this.ensureDeviceConnected();
    this.ensureWalletUnlocked();
    
    try {
      const result = await this.transactionSigner!.signTransaction(
        transaction,
        this.currentWallet!,
        this.teeDevice!,
        options
      );
      
      if (result.txHash) {
        this.emit('transaction:sent', result.txHash);
      }
      
      return result;
    } catch (error) {
      const sdkError = this.wrapError(error, 'signTransaction');
      this.emit('transaction:failed', sdkError);
      throw sdkError;
    }
  }

  /**
   * 签名消息
   */
  async signMessage(message: string): Promise<string> {
    this.ensureInitialized();
    this.ensureDeviceConnected();
    this.ensureWalletUnlocked();
    
    try {
      return await this.transactionSigner!.signMessage(
        message,
        this.currentWallet!,
        this.teeDevice!
      );
    } catch (error) {
      throw this.wrapError(error, 'signMessage');
    }
  }

  // === 工具方法 ===

  /**
   * 获取版本信息
   */
  getVersion(): VersionInfo {
    return {
      sdkVersion: VERSION_INFO.SDK_VERSION,
      coreVersion: '0.1.0', // 从core-logic获取
      apiVersion: VERSION_INFO.API_VERSION,
      firmwareVersion: this.currentDevice?.version
    };
  }

  /**
   * 获取配置信息
   */
  getConfig(): AirAccountConfig {
    return { ...this.config };
  }

  /**
   * 更新配置
   */
  async updateConfig(newConfig: Partial<AirAccountConfig>): Promise<void> {
    this.config = {
      ...this.config,
      ...newConfig,
      teeConfig: {
        ...this.config.teeConfig,
        ...newConfig.teeConfig
      }
    };
    
    // 如果已初始化，需要重新初始化相关组件
    if (this.isInitialized) {
      await this.reinitializeComponents();
    }
  }

  // === 私有方法 ===

  private validateConfig(): void {
    if (!this.config.teeConfig) {
      throw createError(ErrorCode.INVALID_CONFIG, '缺少TEE设备配置');
    }
    
    if (!this.config.apiEndpoint) {
      throw createError(ErrorCode.INVALID_CONFIG, '缺少API端点配置');
    }
  }

  private setupEventForwarding(): void {
    if (this.teeDevice) {
      this.teeDevice.on('device:error', (error) => {
        this.emit('device:error', error);
      });
    }
  }

  private async reinitializeComponents(): Promise<void> {
    // 重新初始化需要更新配置的组件
    if (this.walletManager) {
      await this.walletManager.updateConfig(this.config);
    }
    
    if (this.teeDevice) {
      await this.teeDevice.updateConfig(this.config.teeConfig!);
    }
  }

  private ensureInitialized(): void {
    if (!this.isInitialized) {
      throw createError(ErrorCode.SDK_NOT_INITIALIZED, 'SDK未初始化，请先调用initialize()');
    }
  }

  private ensureDeviceConnected(): void {
    if (!this.isDeviceConnected()) {
      throw createError(ErrorCode.DEVICE_NOT_FOUND, '未连接TEE设备');
    }
  }

  private ensureWalletUnlocked(): void {
    if (!this.currentWallet) {
      throw createError(ErrorCode.WALLET_NOT_FOUND, '未选择钱包');
    }
    
    if (this.currentWallet.status === 'locked') {
      throw createError(ErrorCode.WALLET_LOCKED, '钱包已锁定，请先解锁');
    }
  }

  private wrapError(error: any, operation: string): AirAccountError {
    if (error instanceof Error && 'code' in error) {
      return error as AirAccountError;
    }
    
    return createError(
      ErrorCode.SDK_NOT_INITIALIZED,
      `SDK操作失败: ${operation} - ${error.message || error}`,
      { operation, originalError: error },
      'sdk'
    );
  }
}