// AirAccount SDK Constants
// 系统常量和配置

import { ChainConfig, ErrorCode } from '../types';

// === 支持的区块链网络 ===
export const SUPPORTED_CHAINS: Record<string, ChainConfig> = {
  ethereum: {
    chainId: 1,
    name: 'Ethereum Mainnet',
    rpcUrls: [
      'https://eth.llamarpc.com',
      'https://rpc.ankr.com/eth',
      'https://ethereum-rpc.publicnode.com'
    ],
    blockExplorerUrls: ['https://etherscan.io'],
    nativeCurrency: {
      name: 'Ether',
      symbol: 'ETH',
      decimals: 18
    }
  },
  polygon: {
    chainId: 137,
    name: 'Polygon Mainnet',
    rpcUrls: [
      'https://polygon.llamarpc.com',
      'https://rpc.ankr.com/polygon',
      'https://polygon-rpc.com'
    ],
    blockExplorerUrls: ['https://polygonscan.com'],
    nativeCurrency: {
      name: 'MATIC',
      symbol: 'MATIC',
      decimals: 18
    }
  },
  arbitrum: {
    chainId: 42161,
    name: 'Arbitrum One',
    rpcUrls: [
      'https://arb1.arbitrum.io/rpc',
      'https://rpc.ankr.com/arbitrum',
      'https://arbitrum.llamarpc.com'
    ],
    blockExplorerUrls: ['https://arbiscan.io'],
    nativeCurrency: {
      name: 'Ether',
      symbol: 'ETH',
      decimals: 18
    }
  },
  optimism: {
    chainId: 10,
    name: 'Optimism',
    rpcUrls: [
      'https://mainnet.optimism.io',
      'https://rpc.ankr.com/optimism',
      'https://optimism.llamarpc.com'
    ],
    blockExplorerUrls: ['https://optimistic.etherscan.io'],
    nativeCurrency: {
      name: 'Ether',
      symbol: 'ETH',
      decimals: 18
    }
  },
  bsc: {
    chainId: 56,
    name: 'BNB Smart Chain',
    rpcUrls: [
      'https://bsc-dataseed1.binance.org',
      'https://rpc.ankr.com/bsc',
      'https://bsc.llamarpc.com'
    ],
    blockExplorerUrls: ['https://bscscan.com'],
    nativeCurrency: {
      name: 'BNB',
      symbol: 'BNB',
      decimals: 18
    }
  },
  // 测试网络
  sepolia: {
    chainId: 11155111,
    name: 'Sepolia',
    rpcUrls: [
      'https://rpc.sepolia.org',
      'https://rpc.ankr.com/eth_sepolia',
      'https://sepolia.infura.io/v3/'
    ],
    blockExplorerUrls: ['https://sepolia.etherscan.io'],
    nativeCurrency: {
      name: 'Ether',
      symbol: 'ETH',
      decimals: 18
    }
  }
};

// === 默认配置 ===
export const DEFAULT_CONFIG = {
  apiEndpoint: 'https://api.airaccount.io',
  network: 'mainnet' as const,
  logLevel: 'info' as const,
  timeout: 30000,
  teeConfig: {
    connectionType: 'bluetooth' as const,
    connectionTimeout: 10000,
    autoReconnect: true,
    reconnectInterval: 5000
  }
};

// === 错误代码映射 ===
export const ERROR_CODES: Record<ErrorCode, string> = {
  // SDK错误
  [ErrorCode.SDK_NOT_INITIALIZED]: 'SDK未初始化',
  [ErrorCode.INVALID_CONFIG]: '无效的配置参数',
  [ErrorCode.INVALID_PARAMETERS]: '无效的参数',
  
  // 设备错误
  [ErrorCode.DEVICE_NOT_FOUND]: '未找到TEE设备',
  [ErrorCode.DEVICE_CONNECTION_FAILED]: '设备连接失败',
  [ErrorCode.DEVICE_DISCONNECTED]: '设备连接断开',
  [ErrorCode.DEVICE_TIMEOUT]: '设备响应超时',
  [ErrorCode.DEVICE_BUSY]: '设备繁忙',
  
  // 钱包错误
  [ErrorCode.WALLET_NOT_FOUND]: '钱包不存在',
  [ErrorCode.WALLET_LOCKED]: '钱包已锁定',
  [ErrorCode.INVALID_MNEMONIC]: '无效的助记词',
  [ErrorCode.INVALID_PASSWORD]: '密码错误',
  [ErrorCode.WALLET_EXISTS]: '钱包已存在',
  
  // 交易错误
  [ErrorCode.TRANSACTION_FAILED]: '交易失败',
  [ErrorCode.INSUFFICIENT_FUNDS]: '余额不足',
  [ErrorCode.INVALID_TRANSACTION]: '无效的交易',
  [ErrorCode.GAS_ESTIMATION_FAILED]: 'Gas估算失败',
  [ErrorCode.SIGNATURE_FAILED]: '签名失败',
  
  // 网络错误
  [ErrorCode.NETWORK_ERROR]: '网络连接错误',
  [ErrorCode.RPC_ERROR]: 'RPC调用错误',
  [ErrorCode.TIMEOUT_ERROR]: '请求超时',
  
  // 安全错误
  [ErrorCode.UNAUTHORIZED]: '未授权访问',
  [ErrorCode.PERMISSION_DENIED]: '权限不足',
  [ErrorCode.SECURITY_VIOLATION]: '安全违规'
};

// === BIP44派生路径 ===
export const DERIVATION_PATHS = {
  ethereum: "m/44'/60'/0'/0/0",
  bitcoin: "m/44'/0'/0'/0/0",
  polygon: "m/44'/60'/0'/0/0",
  arbitrum: "m/44'/60'/0'/0/0",
  optimism: "m/44'/60'/0'/0/0",
  bsc: "m/44'/60'/0'/0/0"
};

// === 交易参数 ===
export const TRANSACTION_DEFAULTS = {
  gasLimit: '21000',
  gasPrice: '20000000000', // 20 Gwei
  confirmations: 1,
  timeout: 60000 // 1分钟
};

// === TEE设备配置 ===
export const TEE_DEVICE_CONSTANTS = {
  // 蓝牙服务UUID
  BLUETOOTH_SERVICE_UUID: '6e400001-b5a3-f393-e0a9-e50e24dcca9e',
  BLUETOOTH_CHARACTERISTIC_TX: '6e400002-b5a3-f393-e0a9-e50e24dcca9e',
  BLUETOOTH_CHARACTERISTIC_RX: '6e400003-b5a3-f393-e0a9-e50e24dcca9e',
  
  // USB设备标识
  USB_VENDOR_ID: 0x1209,
  USB_PRODUCT_ID: 0x5741,
  
  // 通信协议版本
  PROTOCOL_VERSION: '1.0.0',
  
  // 命令超时时间
  COMMAND_TIMEOUT: 10000,
  
  // 最大重试次数
  MAX_RETRIES: 3
};

// === API端点 ===
export const API_ENDPOINTS = {
  mainnet: {
    base: 'https://api.airaccount.io',
    websocket: 'wss://ws.airaccount.io'
  },
  testnet: {
    base: 'https://testnet-api.airaccount.io',
    websocket: 'wss://testnet-ws.airaccount.io'
  },
  devnet: {
    base: 'http://localhost:3000',
    websocket: 'ws://localhost:3001'
  }
};

// === 安全配置 ===
export const SECURITY_CONFIG = {
  // 会话超时时间（毫秒）
  SESSION_TIMEOUT: 15 * 60 * 1000, // 15分钟
  
  // 最大失败尝试次数
  MAX_FAILED_ATTEMPTS: 5,
  
  // 锁定时间（毫秒）
  LOCKOUT_DURATION: 30 * 60 * 1000, // 30分钟
  
  // 加密算法
  ENCRYPTION_ALGORITHM: 'AES-256-GCM',
  
  // 密钥长度
  KEY_LENGTH: 32,
  
  // IV长度
  IV_LENGTH: 16
};

// === 版本信息 ===
export const VERSION_INFO = {
  SDK_VERSION: '0.1.0-alpha.1',
  API_VERSION: '1.0',
  MIN_FIRMWARE_VERSION: '1.0.0'
};

// === 支持的语言 ===
export const SUPPORTED_LANGUAGES = [
  'zh-CN',
  'en-US',
  'ja-JP',
  'ko-KR',
  'de-DE',
  'fr-FR',
  'es-ES'
] as const;

export type SupportedLanguage = typeof SUPPORTED_LANGUAGES[number];

// === 缓存配置 ===
export const CACHE_CONFIG = {
  // 默认TTL（秒）
  DEFAULT_TTL: 300, // 5分钟
  
  // 最大缓存大小
  MAX_CACHE_SIZE: 1000,
  
  // 余额缓存TTL
  BALANCE_CACHE_TTL: 30, // 30秒
  
  // 交易历史缓存TTL
  TRANSACTION_CACHE_TTL: 60, // 1分钟
  
  // 设备信息缓存TTL
  DEVICE_INFO_CACHE_TTL: 600 // 10分钟
};