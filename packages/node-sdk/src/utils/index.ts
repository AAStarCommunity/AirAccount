// AirAccount SDK Utilities
// 通用工具函数

import { ethers } from 'ethers';
import { isAddress, parseEther, formatEther, parseGwei, formatGwei } from 'viem';
import { ErrorCode, AirAccountError } from '../types';

// === 助记词处理工具 ===

/**
 * 验证BIP39助记词
 */
export function validateMnemonic(mnemonic: string): boolean {
  try {
    return ethers.Mnemonic.isValidMnemonic(mnemonic);
  } catch {
    return false;
  }
}

/**
 * 生成BIP39助记词
 */
export function generateMnemonic(strength: number = 128): string {
  try {
    const entropy = ethers.randomBytes(strength / 8);
    return ethers.Mnemonic.fromEntropy(entropy).phrase;
  } catch (error) {
    throw createError(ErrorCode.INVALID_PARAMETERS, '无效的助记词强度');
  }
}

// === 地址格式化工具 ===

/**
 * 格式化以太坊地址
 */
export function formatAddress(address: string, length: number = 8): string {
  if (!isValidAddress(address)) {
    return address;
  }
  
  if (length >= 40) {
    return address;
  }
  
  const start = address.slice(0, 2 + Math.ceil(length / 2));
  const end = address.slice(-Math.floor(length / 2));
  
  return `${start}...${end}`;
}

/**
 * 验证以太坊地址格式
 */
export function isValidAddress(address: string): boolean {
  return isAddress(address);
}

/**
 * 地址校验和检查
 */
export function toChecksumAddress(address: string): string {
  if (!isValidAddress(address)) {
    throw createError(ErrorCode.INVALID_PARAMETERS, '无效的地址格式');
  }
  
  return ethers.getAddress(address);
}

// === 交易处理工具 ===

/**
 * 解析交易数据
 */
export function parseTransaction(txData: string): {
  method?: string;
  params?: any[];
  value?: string;
} {
  if (!txData || !txData.startsWith('0x')) {
    return {};
  }
  
  const data = txData.slice(2);
  
  if (data.length === 0) {
    return { method: 'transfer' };
  }
  
  // 提取方法签名（前4字节）
  const methodSignature = data.slice(0, 8);
  
  // 常见方法签名映射
  const methodMap: Record<string, string> = {
    'a9059cbb': 'transfer',
    '095ea7b3': 'approve',
    '23b872dd': 'transferFrom',
    '40c10f19': 'mint',
    '42966c68': 'burn'
  };
  
  const method = methodMap[methodSignature] || 'unknown';
  
  return {
    method,
    params: [], // 可以使用ethers.Interface进一步解析
    value: '0'
  };
}

/**
 * 计算交易手续费
 */
export function calculateTransactionFee(gasLimit: string, gasPrice: string): string {
  try {
    const limit = BigInt(gasLimit);
    const price = BigInt(gasPrice);
    const fee = limit * price;
    
    return fee.toString();
  } catch (error) {
    throw createError(ErrorCode.INVALID_PARAMETERS, '无效的Gas参数');
  }
}

/**
 * 格式化Wei为Ether
 */
export function formatWeiToEther(wei: string, decimals: number = 6): string {
  try {
    const formatted = formatEther(BigInt(wei));
    return parseFloat(formatted).toFixed(decimals);
  } catch (error) {
    return '0';
  }
}

/**
 * 格式化Ether为Wei
 */
export function formatEtherToWei(ether: string): string {
  try {
    return parseEther(ether).toString();
  } catch (error) {
    throw createError(ErrorCode.INVALID_PARAMETERS, '无效的Ether数值');
  }
}

/**
 * 格式化Gwei
 */
export function formatGweiToWei(gwei: string): string {
  try {
    return parseGwei(gwei).toString();
  } catch (error) {
    throw createError(ErrorCode.INVALID_PARAMETERS, '无效的Gwei数值');
  }
}

/**
 * 格式化Wei为Gwei
 */
export function formatWeiToGwei(wei: string): string {
  try {
    return formatGwei(BigInt(wei));
  } catch (error) {
    return '0';
  }
}

// === 错误处理工具 ===

/**
 * 创建标准化错误
 */
export function createError(
  code: ErrorCode,
  message: string,
  details?: Record<string, any>,
  source?: 'sdk' | 'device' | 'network' | 'user'
): AirAccountError {
  const error = new Error(message) as AirAccountError;
  error.name = 'AirAccountError';
  error.code = code;
  error.details = details;
  error.source = source;
  
  return error;
}

/**
 * 判断是否为AirAccount错误
 */
export function isAirAccountError(error: any): error is AirAccountError {
  return error && error.name === 'AirAccountError' && error.code;
}

// === 数据验证工具 ===

/**
 * 验证URL格式
 */
export function isValidUrl(url: string): boolean {
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
}

/**
 * 验证链ID
 */
export function isValidChainId(chainId: number): boolean {
  return Number.isInteger(chainId) && chainId > 0;
}

/**
 * 验证十六进制字符串
 */
export function isValidHex(hex: string): boolean {
  if (!hex.startsWith('0x')) return false;
  return /^0x[a-fA-F0-9]*$/.test(hex);
}

// === 时间处理工具 ===

/**
 * 格式化时间戳
 */
export function formatTimestamp(timestamp: number, locale: string = 'zh-CN'): string {
  const date = new Date(timestamp * 1000);
  
  return date.toLocaleString(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  });
}

/**
 * 获取相对时间描述
 */
export function getRelativeTime(timestamp: number, locale: string = 'zh-CN'): string {
  const now = Date.now();
  const diff = now - (timestamp * 1000);
  
  const minute = 60 * 1000;
  const hour = 60 * minute;
  const day = 24 * hour;
  
  if (diff < minute) {
    return '刚刚';
  } else if (diff < hour) {
    const minutes = Math.floor(diff / minute);
    return `${minutes}分钟前`;
  } else if (diff < day) {
    const hours = Math.floor(diff / hour);
    return `${hours}小时前`;
  } else {
    const days = Math.floor(diff / day);
    return `${days}天前`;
  }
}

// === 数据处理工具 ===

/**
 * 深度克隆对象
 */
export function deepClone<T>(obj: T): T {
  if (obj === null || typeof obj !== 'object') {
    return obj;
  }
  
  if (obj instanceof Date) {
    return new Date(obj.getTime()) as unknown as T;
  }
  
  if (obj instanceof Array) {
    return obj.map(item => deepClone(item)) as unknown as T;
  }
  
  const cloned = {} as T;
  for (const key in obj) {
    if (obj.hasOwnProperty(key)) {
      cloned[key] = deepClone(obj[key]);
    }
  }
  
  return cloned;
}

/**
 * 安全的JSON解析
 */
export function safeJsonParse<T>(json: string, defaultValue: T): T {
  try {
    return JSON.parse(json) as T;
  } catch {
    return defaultValue;
  }
}

/**
 * 节流函数
 */
export function throttle<T extends (...args: any[]) => any>(
  func: T,
  delay: number
): (...args: Parameters<T>) => void {
  let lastCall = 0;
  
  return (...args: Parameters<T>) => {
    const now = Date.now();
    if (now - lastCall >= delay) {
      lastCall = now;
      func(...args);
    }
  };
}

/**
 * 防抖函数
 */
export function debounce<T extends (...args: any[]) => any>(
  func: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: NodeJS.Timeout;
  
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => func(...args), delay);
  };
}

// === 加密工具（示例，实际使用专业库） ===

/**
 * 生成随机字符串
 */
export function generateRandomString(length: number = 32): string {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let result = '';
  
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  
  return result;
}

/**
 * 简单的字符串哈希
 */
export function simpleHash(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = ((hash << 5) - hash) + char;
    hash = hash & hash; // 转换为32位整数
  }
  
  return Math.abs(hash).toString(16);
}

// === 设备检测工具 ===

/**
 * 检测运行环境
 */
export function getEnvironmentInfo(): {
  isNode: boolean;
  isBrowser: boolean;
  isElectron: boolean;
  platform?: string;
} {
  const isNode = typeof process !== 'undefined' && 
                 process.versions && 
                 process.versions.node;
  
  const isBrowser = typeof window !== 'undefined' && 
                    typeof window.document !== 'undefined';
  
  const isElectron = typeof process !== 'undefined' && 
                     process.versions && 
                     process.versions.electron;
  
  return {
    isNode: !!isNode,
    isBrowser,
    isElectron: !!isElectron,
    platform: typeof process !== 'undefined' ? process.platform : undefined
  };
}