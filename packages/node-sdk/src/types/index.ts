// AirAccount SDK Types
// 提供完整的TypeScript类型定义

import { EventEmitter } from 'eventemitter3';

// === Core Configuration Types ===
export interface AirAccountConfig {
  /** API端点配置 */
  apiEndpoint?: string;
  /** TEE设备连接配置 */
  teeConfig?: TEEDeviceConfig;
  /** 网络配置 */
  network?: 'mainnet' | 'testnet' | 'devnet';
  /** 日志级别 */
  logLevel?: 'debug' | 'info' | 'warn' | 'error';
  /** 超时配置 */
  timeout?: number;
}

// === Device Types ===
export interface TEEDeviceConfig {
  /** 连接类型 */
  connectionType: DeviceConnectionType;
  /** 设备ID */
  deviceId?: string;
  /** 连接超时时间 */
  connectionTimeout?: number;
  /** 自动重连 */
  autoReconnect?: boolean;
  /** 重连间隔 */
  reconnectInterval?: number;
}

export type DeviceConnectionType = 'bluetooth' | 'usb' | 'wifi' | 'serial';

export interface DeviceInfo {
  /** 设备ID */
  id: string;
  /** 设备名称 */
  name: string;
  /** 设备类型 */
  type: string;
  /** 连接状态 */
  state: DeviceState;
  /** 设备版本 */
  version?: string;
  /** 电池电量 */
  batteryLevel?: number;
  /** 信号强度 */
  signalStrength?: number;
}

export type DeviceState = 'disconnected' | 'connecting' | 'connected' | 'pairing' | 'error';

// === Wallet Types ===
export interface WalletInfo {
  /** 钱包地址 */
  address: string;
  /** 公钥 */
  publicKey: string;
  /** 钱包状态 */
  status: WalletStatus;
  /** 支持的链 */
  supportedChains: string[];
  /** 创建时间 */
  createdAt: Date;
  /** 最后访问时间 */
  lastAccessedAt?: Date;
}

export type WalletStatus = 'locked' | 'unlocked' | 'initializing' | 'error';

export interface CreateWalletOptions {
  /** 助记词强度 */
  strength?: 128 | 160 | 192 | 224 | 256;
  /** 密码保护 */
  password?: string;
  /** 钱包名称 */
  name?: string;
  /** 派生路径 */
  derivationPath?: string;
}

export interface ImportWalletOptions {
  /** 助记词 */
  mnemonic: string;
  /** 密码保护 */
  password?: string;
  /** 钱包名称 */
  name?: string;
  /** 派生路径 */
  derivationPath?: string;
  /** 验证助记词 */
  validate?: boolean;
}

export interface Balance {
  /** 原生代币余额 */
  native: string;
  /** ERC-20代币余额 */
  tokens: TokenBalance[];
  /** NFT数量 */
  nftCount: number;
  /** 总价值(USD) */
  totalValueUsd?: string;
}

export interface TokenBalance {
  /** 代币合约地址 */
  contractAddress: string;
  /** 代币符号 */
  symbol: string;
  /** 代币名称 */
  name: string;
  /** 余额 */
  balance: string;
  /** 小数位数 */
  decimals: number;
  /** 价格(USD) */
  priceUsd?: string;
  /** 价值(USD) */
  valueUsd?: string;
}

// === Transaction Types ===
export interface TransactionRequest {
  /** 目标地址 */
  to: string;
  /** 转账金额 */
  value: string;
  /** 数据 */
  data?: string;
  /** Gas限制 */
  gasLimit?: string;
  /** Gas价格 */
  gasPrice?: string;
  /** Nonce */
  nonce?: number;
  /** 链ID */
  chainId: number;
  /** 交易类型 */
  type?: TransactionType;
}

export type TransactionType = 'legacy' | 'eip1559' | 'eip2930';

export interface SignTransactionOptions {
  /** 是否广播交易 */
  broadcast?: boolean;
  /** 等待确认 */
  waitForConfirmation?: boolean;
  /** 确认数量 */
  confirmations?: number;
  /** 超时时间 */
  timeout?: number;
}

export interface SignatureResponse {
  /** 签名结果 */
  signature: string;
  /** 交易哈希 */
  txHash?: string;
  /** 已广播 */
  broadcasted?: boolean;
  /** 确认状态 */
  confirmed?: boolean;
}

// === Chain Configuration ===
export interface ChainConfig {
  /** 链ID */
  chainId: number;
  /** 链名称 */
  name: string;
  /** RPC端点 */
  rpcUrls: string[];
  /** 浏览器URL */
  blockExplorerUrls?: string[];
  /** 原生货币 */
  nativeCurrency: {
    name: string;
    symbol: string;
    decimals: number;
  };
}

// === Error Types ===
export interface AirAccountError extends Error {
  /** 错误代码 */
  code: ErrorCode;
  /** 错误详情 */
  details?: Record<string, any>;
  /** 错误源 */
  source?: 'sdk' | 'device' | 'network' | 'user';
}

export enum ErrorCode {
  // SDK错误
  SDK_NOT_INITIALIZED = 'SDK_NOT_INITIALIZED',
  INVALID_CONFIG = 'INVALID_CONFIG',
  INVALID_PARAMETERS = 'INVALID_PARAMETERS',
  
  // 设备错误
  DEVICE_NOT_FOUND = 'DEVICE_NOT_FOUND',
  DEVICE_CONNECTION_FAILED = 'DEVICE_CONNECTION_FAILED',
  DEVICE_DISCONNECTED = 'DEVICE_DISCONNECTED',
  DEVICE_TIMEOUT = 'DEVICE_TIMEOUT',
  DEVICE_BUSY = 'DEVICE_BUSY',
  
  // 钱包错误
  WALLET_NOT_FOUND = 'WALLET_NOT_FOUND',
  WALLET_LOCKED = 'WALLET_LOCKED',
  INVALID_MNEMONIC = 'INVALID_MNEMONIC',
  INVALID_PASSWORD = 'INVALID_PASSWORD',
  WALLET_EXISTS = 'WALLET_EXISTS',
  
  // 交易错误
  TRANSACTION_FAILED = 'TRANSACTION_FAILED',
  INSUFFICIENT_FUNDS = 'INSUFFICIENT_FUNDS',
  INVALID_TRANSACTION = 'INVALID_TRANSACTION',
  GAS_ESTIMATION_FAILED = 'GAS_ESTIMATION_FAILED',
  SIGNATURE_FAILED = 'SIGNATURE_FAILED',
  
  // 网络错误
  NETWORK_ERROR = 'NETWORK_ERROR',
  RPC_ERROR = 'RPC_ERROR',
  TIMEOUT_ERROR = 'TIMEOUT_ERROR',
  
  // 安全错误
  UNAUTHORIZED = 'UNAUTHORIZED',
  PERMISSION_DENIED = 'PERMISSION_DENIED',
  SECURITY_VIOLATION = 'SECURITY_VIOLATION',
}

// === Event Types ===
export interface SDKEvents {
  // 连接事件
  'device:connected': (device: DeviceInfo) => void;
  'device:disconnected': (device: DeviceInfo) => void;
  'device:error': (error: AirAccountError) => void;
  
  // 钱包事件
  'wallet:created': (wallet: WalletInfo) => void;
  'wallet:unlocked': (wallet: WalletInfo) => void;
  'wallet:locked': (wallet: WalletInfo) => void;
  
  // 交易事件
  'transaction:sent': (txHash: string) => void;
  'transaction:confirmed': (txHash: string, confirmations: number) => void;
  'transaction:failed': (error: AirAccountError) => void;
  
  // 一般事件
  'ready': () => void;
  'error': (error: AirAccountError) => void;
}

// === Utility Types ===
export type EventMap = SDKEvents;
export type EventKey = keyof EventMap;
export type EventReceiver<T> = (params: T) => void;

export interface SDKEventEmitter extends EventEmitter<EventMap> {}

// === Version Info ===
export interface VersionInfo {
  /** SDK版本 */
  sdkVersion: string;
  /** 核心逻辑版本 */
  coreVersion: string;
  /** API版本 */
  apiVersion: string;
  /** 设备固件版本 */
  firmwareVersion?: string;
}