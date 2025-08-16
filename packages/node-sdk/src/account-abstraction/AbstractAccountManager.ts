/**
 * 账户抽象管理器 - 基于ERC-4337和all-about-abstract-account参考
 * 
 * 参考资料：
 * - https://github.com/mingder78/all-about-abstract-account
 * - ERC-4337 Account Abstraction标准
 */

import { ethers } from 'ethers';
import { WebAuthnManager } from '../webauthn/WebAuthnManager';

export interface UserOperation {
  sender: string;
  nonce: string;
  initCode: string;
  callData: string;
  callGasLimit: string;
  verificationGasLimit: string;
  preVerificationGas: string;
  maxFeePerGas: string;
  maxPriorityFeePerGas: string;
  paymasterAndData: string;
  signature: string;
}

export interface AbstractAccountConfig {
  entryPointAddress: string;
  factoryAddress: string;
  paymasterUrl?: string;
  bundlerUrl: string;
  chainId: number;
}

export interface AccountInfo {
  address: string;
  nonce: number;
  isDeployed: boolean;
  owner: string;
  recoveryMethod: 'passkey' | 'guardian' | 'social';
}

export interface TransactionRequest {
  to: string;
  value?: string;
  data?: string;
  gasLimit?: string;
}

/**
 * 抽象账户管理器
 * 整合WebAuthn认证和ERC-4337账户抽象
 */
export class AbstractAccountManager {
  private config: AbstractAccountConfig;
  private webauthnManager: WebAuthnManager;
  private provider: ethers.Provider;
  private currentAccount: AccountInfo | null = null;

  constructor(
    config: AbstractAccountConfig,
    webauthnManager: WebAuthnManager,
    provider: ethers.Provider
  ) {
    this.config = config;
    this.webauthnManager = webauthnManager;
    this.provider = provider;
  }

  /**
   * 创建新的抽象账户（通过WebAuthn认证）
   * 
   * @param user 用户信息
   * @param options 创建选项
   * @returns 账户信息
   */
  async createAccount(user: { email: string; displayName: string }, options: {
    salt?: string;
    initialDeposit?: string;
    recoveryGuardians?: string[];
  } = {}): Promise<AccountInfo> {
    // 步骤1: 通过WebAuthn注册用户
    const webauthnResult = await this.webauthnManager.registerPasskey({
      id: ethers.id(user.email), // 使用email生成确定性ID
      email: user.email,
      displayName: user.displayName
    });

    if (!webauthnResult.success) {
      throw new Error(`WebAuthn registration failed: ${webauthnResult.error}`);
    }

    // 步骤2: 从WebAuthn凭证派生账户地址
    const credentialId = webauthnResult.credential!.credentialId;
    const salt = options.salt || ethers.id(credentialId);
    const accountAddress = await this.computeAccountAddress(credentialId, salt);

    // 步骤3: 准备账户部署
    const initCode = await this.generateInitCode(credentialId, salt, options.recoveryGuardians);
    
    // 步骤4: 如果有初始存款，创建部署UserOperation
    if (options.initialDeposit) {
      await this.deployAccountWithDeposit(accountAddress, initCode, options.initialDeposit);
    }

    const accountInfo: AccountInfo = {
      address: accountAddress,
      nonce: 0,
      isDeployed: Boolean(options.initialDeposit),
      owner: credentialId,
      recoveryMethod: 'passkey'
    };

    this.currentAccount = accountInfo;
    return accountInfo;
  }

  /**
   * 通过WebAuthn认证恢复账户访问
   * 
   * @param email 用户邮箱
   * @returns 账户信息
   */
  async recoverAccount(email: string): Promise<AccountInfo> {
    // 步骤1: 通过WebAuthn认证用户
    const authResult = await this.webauthnManager.authenticateWithPasskey(email);

    if (!authResult.success) {
      throw new Error(`WebAuthn authentication failed: ${authResult.error}`);
    }

    // 步骤2: 从服务器获取账户信息
    const accountInfo = await this.getAccountFromServer(authResult.sessionToken!);
    
    // 步骤3: 验证账户状态
    const onChainNonce = await this.getAccountNonce(accountInfo.address);
    accountInfo.nonce = onChainNonce;
    accountInfo.isDeployed = onChainNonce > 0;

    this.currentAccount = accountInfo;
    return accountInfo;
  }

  /**
   * 执行交易（通过WebAuthn签名）
   * 
   * @param transaction 交易请求
   * @param usePaymaster 是否使用Paymaster代付gas
   * @returns 交易哈希
   */
  async executeTransaction(
    transaction: TransactionRequest,
    usePaymaster: boolean = false
  ): Promise<string> {
    if (!this.currentAccount) {
      throw new Error('No account selected');
    }

    if (!this.webauthnManager.isAuthenticated()) {
      throw new Error('User not authenticated');
    }

    // 步骤1: 构建UserOperation
    const userOp = await this.buildUserOperation(transaction, usePaymaster);

    // 步骤2: 通过TEE + WebAuthn签名
    const signature = await this.signUserOperation(userOp);
    userOp.signature = signature;

    // 步骤3: 提交到Bundler
    const txHash = await this.submitUserOperation(userOp);

    // 步骤4: 等待交易确认
    await this.waitForTransactionReceipt(txHash);

    return txHash;
  }

  /**
   * 批量执行交易
   * 
   * @param transactions 批量交易请求
   * @param usePaymaster 是否使用Paymaster
   * @returns 交易哈希
   */
  async executeBatch(
    transactions: TransactionRequest[],
    usePaymaster: boolean = false
  ): Promise<string> {
    if (!this.currentAccount) {
      throw new Error('No account selected');
    }

    // 构建批量调用数据
    const batchCallData = this.encodeBatchCall(transactions);
    
    const batchTransaction: TransactionRequest = {
      to: this.currentAccount.address,
      data: batchCallData
    };

    return await this.executeTransaction(batchTransaction, usePaymaster);
  }

  /**
   * 获取账户余额
   * 
   * @param address 账户地址（可选）
   * @returns 余额信息
   */
  async getAccountBalance(address?: string): Promise<{
    native: string;
    tokens: { address: string; symbol: string; balance: string; }[];
  }> {
    const accountAddress = address || this.currentAccount?.address;
    if (!accountAddress) {
      throw new Error('No account address provided');
    }

    const nativeBalance = await this.provider.getBalance(accountAddress);
    
    // TODO: 获取ERC-20代币余额
    const tokens: { address: string; symbol: string; balance: string; }[] = [];

    return {
      native: ethers.formatEther(nativeBalance),
      tokens
    };
  }

  /**
   * 获取当前账户信息
   */
  getCurrentAccount(): AccountInfo | null {
    return this.currentAccount;
  }

  // === 私有方法 ===

  /**
   * 计算账户地址
   */
  private async computeAccountAddress(credentialId: string, salt: string): Promise<string> {
    // 使用CREATE2计算确定性地址
    const initCodeHash = ethers.keccak256(
      ethers.solidityPacked(
        ['bytes', 'bytes32'],
        [await this.generateInitCode(credentialId, salt), salt]
      )
    );

    return ethers.getCreate2Address(
      this.config.factoryAddress,
      salt,
      initCodeHash
    );
  }

  /**
   * 生成初始化代码
   */
  private async generateInitCode(
    credentialId: string, 
    salt: string, 
    guardians?: string[]
  ): Promise<string> {
    // 构建工厂合约调用数据
    const factoryInterface = new ethers.Interface([
      'function createAccount(string calldata credentialId, bytes32 salt, address[] calldata guardians) returns (address)'
    ]);

    const createAccountData = factoryInterface.encodeFunctionData('createAccount', [
      credentialId,
      salt,
      guardians || []
    ]);

    return ethers.concat([this.config.factoryAddress, createAccountData]);
  }

  /**
   * 构建UserOperation
   */
  private async buildUserOperation(
    transaction: TransactionRequest,
    usePaymaster: boolean
  ): Promise<UserOperation> {
    if (!this.currentAccount) {
      throw new Error('No account selected');
    }

    const nonce = await this.getAccountNonce(this.currentAccount.address);
    
    // 构建调用数据
    const callData = this.encodeCall(transaction);

    // 估算gas
    const gasEstimate = await this.estimateUserOperationGas(callData);

    // 获取gas价格
    const feeData = await this.provider.getFeeData();

    let paymasterAndData = '0x';
    if (usePaymaster && this.config.paymasterUrl) {
      paymasterAndData = await this.getPaymasterData(callData, gasEstimate);
    }

    return {
      sender: this.currentAccount.address,
      nonce: `0x${nonce.toString(16)}`,
      initCode: this.currentAccount.isDeployed ? '0x' : await this.generateInitCode(this.currentAccount.owner, ethers.id(this.currentAccount.owner)),
      callData,
      callGasLimit: `0x${gasEstimate.callGasLimit.toString(16)}`,
      verificationGasLimit: `0x${gasEstimate.verificationGasLimit.toString(16)}`,
      preVerificationGas: `0x${gasEstimate.preVerificationGas.toString(16)}`,
      maxFeePerGas: `0x${feeData.maxFeePerGas?.toString(16) || '0'}`,
      maxPriorityFeePerGas: `0x${feeData.maxPriorityFeePerGas?.toString(16) || '0'}`,
      paymasterAndData,
      signature: '0x' // 稍后填充
    };
  }

  /**
   * 签名UserOperation（集成WebAuthn + TEE）
   */
  private async signUserOperation(userOp: UserOperation): Promise<string> {
    // 计算UserOperation哈希
    const userOpHash = this.getUserOperationHash(userOp);

    // 通过TEE + WebAuthn签名
    // 这里应该调用TEE服务进行签名
    // 暂时返回模拟签名
    const mockSignature = ethers.Signature.from({
      r: '0x' + '1'.repeat(64),
      s: '0x' + '2'.repeat(64),
      v: 27
    });

    return mockSignature.serialized;
  }

  /**
   * 提交UserOperation到Bundler
   */
  private async submitUserOperation(userOp: UserOperation): Promise<string> {
    const response = await fetch(this.config.bundlerUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'eth_sendUserOperation',
        params: [userOp, this.config.entryPointAddress]
      })
    });

    const data = await response.json();
    
    if (data.error) {
      throw new Error(`Bundler error: ${data.error.message}`);
    }

    return data.result;
  }

  /**
   * 编码函数调用
   */
  private encodeCall(transaction: TransactionRequest): string {
    const accountInterface = new ethers.Interface([
      'function execute(address to, uint256 value, bytes calldata data)'
    ]);

    return accountInterface.encodeFunctionData('execute', [
      transaction.to,
      transaction.value || '0',
      transaction.data || '0x'
    ]);
  }

  /**
   * 编码批量调用
   */
  private encodeBatchCall(transactions: TransactionRequest[]): string {
    const accountInterface = new ethers.Interface([
      'function executeBatch(address[] calldata to, uint256[] calldata value, bytes[] calldata data)'
    ]);

    const addresses = transactions.map(tx => tx.to);
    const values = transactions.map(tx => tx.value || '0');
    const dataArray = transactions.map(tx => tx.data || '0x');

    return accountInterface.encodeFunctionData('executeBatch', [addresses, values, dataArray]);
  }

  /**
   * 获取账户nonce
   */
  private async getAccountNonce(address: string): Promise<number> {
    // 从EntryPoint合约获取nonce
    const entryPointInterface = new ethers.Interface([
      'function getNonce(address sender, uint192 key) view returns (uint256 nonce)'
    ]);

    const entryPoint = new ethers.Contract(
      this.config.entryPointAddress,
      entryPointInterface,
      this.provider
    );

    const nonce = await entryPoint.getNonce(address, 0);
    return Number(nonce);
  }

  /**
   * 估算UserOperation gas费用
   */
  private async estimateUserOperationGas(callData: string): Promise<{
    callGasLimit: number;
    verificationGasLimit: number;
    preVerificationGas: number;
  }> {
    // 简化的gas估算，实际应该调用Bundler的估算接口
    return {
      callGasLimit: 100000,
      verificationGasLimit: 150000,
      preVerificationGas: 21000
    };
  }

  /**
   * 获取Paymaster数据
   */
  private async getPaymasterData(callData: string, gasEstimate: any): Promise<string> {
    if (!this.config.paymasterUrl) {
      return '0x';
    }

    // 调用Paymaster服务获取sponsor数据
    // 这里返回模拟数据
    return '0x';
  }

  /**
   * 计算UserOperation哈希
   */
  private getUserOperationHash(userOp: UserOperation): string {
    // 根据ERC-4337标准计算UserOperation哈希
    const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
      ['address', 'uint256', 'bytes32', 'bytes32', 'uint256', 'uint256', 'uint256', 'uint256', 'uint256', 'bytes32'],
      [
        userOp.sender,
        userOp.nonce,
        ethers.keccak256(userOp.initCode),
        ethers.keccak256(userOp.callData),
        userOp.callGasLimit,
        userOp.verificationGasLimit,
        userOp.preVerificationGas,
        userOp.maxFeePerGas,
        userOp.maxPriorityFeePerGas,
        ethers.keccak256(userOp.paymasterAndData)
      ]
    );

    return ethers.keccak256(
      ethers.AbiCoder.defaultAbiCoder().encode(
        ['bytes32', 'address', 'uint256'],
        [ethers.keccak256(encoded), this.config.entryPointAddress, this.config.chainId]
      )
    );
  }

  /**
   * 从服务器获取账户信息
   */
  private async getAccountFromServer(sessionToken: string): Promise<AccountInfo> {
    // 从CA服务器获取账户信息
    // 这里返回模拟数据
    return {
      address: '0x' + '0'.repeat(40),
      nonce: 0,
      isDeployed: false,
      owner: 'mock_credential_id',
      recoveryMethod: 'passkey'
    };
  }

  /**
   * 等待交易收据
   */
  private async waitForTransactionReceipt(txHash: string): Promise<void> {
    let attempts = 0;
    const maxAttempts = 60; // 最多等待5分钟
    
    while (attempts < maxAttempts) {
      try {
        const receipt = await this.provider.getTransactionReceipt(txHash);
        if (receipt) {
          return;
        }
      } catch (error) {
        // 继续等待
      }
      
      await new Promise(resolve => setTimeout(resolve, 5000)); // 等待5秒
      attempts++;
    }
    
    throw new Error('Transaction receipt timeout');
  }

  /**
   * 部署账户并存入初始资金
   */
  private async deployAccountWithDeposit(
    accountAddress: string,
    initCode: string,
    deposit: string
  ): Promise<void> {
    // 创建部署UserOperation
    const deployUserOp: UserOperation = {
      sender: accountAddress,
      nonce: '0x0',
      initCode,
      callData: '0x',
      callGasLimit: '0x30d40', // 200000
      verificationGasLimit: '0x493e0', // 300000
      preVerificationGas: '0x5208', // 21000
      maxFeePerGas: '0x59682f00', // 1.5 gwei
      maxPriorityFeePerGas: '0x3b9aca00', // 1 gwei
      paymasterAndData: '0x',
      signature: '0x'
    };

    const signature = await this.signUserOperation(deployUserOp);
    deployUserOp.signature = signature;

    await this.submitUserOperation(deployUserOp);
  }
}