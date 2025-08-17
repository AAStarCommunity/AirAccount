/**
 * WebAuthn 服务 - 使用 Simple WebAuthn
 * 提供完整的 WebAuthn 注册和认证支持
 */

import {
  generateRegistrationOptions,
  verifyRegistrationResponse,
  generateAuthenticationOptions,
  verifyAuthenticationResponse,
  type GenerateRegistrationOptionsOpts,
  type GenerateAuthenticationOptionsOpts,
  type VerifyRegistrationResponseOpts,
  type VerifyAuthenticationResponseOpts
} from '@simplewebauthn/server';

import {
  type AuthenticatorDevice,
  type RegistrationResponseJSON,
  type AuthenticationResponseJSON
} from '@simplewebauthn/types';

import { WebAuthnError, handleWebAuthnError } from './webauthn-errors.js';
import { 
  Database, 
  type StoredPasskey, 
  type RegistrationState, 
  type AuthenticationState 
} from './database.js';

export interface WebAuthnConfig {
  rpName: string;
  rpID: string;
  origin: string;
}

export interface UserAccount {
  id: string;
  username: string;
  displayName: string;
  devices: AuthenticatorDevice[];
}

export interface Challenge {
  challenge: string;
  userId: string;
  timestamp: number;
  expiresAt: number;
}

export class WebAuthnService {
  private config: WebAuthnConfig;
  private database: Database;
  private isTestMode: boolean;

  constructor(config: WebAuthnConfig, database: Database, isTestMode: boolean = false) {
    this.config = config;
    this.database = database;
    this.isTestMode = isTestMode;
  }

  /**
   * 生成注册选项
   */
  async generateRegistrationOptions(user: { id: string; username: string; displayName: string }) {
    try {
      // 检查是否已有注册状态正在进行
      const existingState = await this.database.getAndRemoveRegistrationState(user.id);
      if (existingState && existingState.expiresAt > Date.now()) {
        throw WebAuthnError.registrationInProgress(user.id);
      }

      // 确保用户在数据库中存在
      await this.database.createOrUpdateUser(user.id, user.username, user.displayName);
      
      // 获取用户现有Passkey（用于排除已注册的设备）
      const existingPasskeys = await this.database.getUserPasskeys(user.id);

      const userVerification = 'preferred';
      const attestationType = 'none';

      const options = await generateRegistrationOptions({
        rpName: this.config.rpName,
        rpID: this.config.rpID,
        userID: user.id,
        userName: user.username,
        userDisplayName: user.displayName,
        attestationType,
        excludeCredentials: existingPasskeys.map(passkey => ({
          id: passkey.credentialId,
          type: 'public-key' as const,
          transports: passkey.transports || [],
        })),
        authenticatorSelection: {
          residentKey: 'preferred',
          userVerification,
          authenticatorAttachment: 'platform', // 优先使用平台认证器（Touch ID, Face ID等）
        },
      });

      // 存储注册状态
      const now = Date.now();
      const registrationState: RegistrationState = {
        userId: user.id,
        challenge: options.challenge,
        userVerification,
        attestation: attestationType,
        createdAt: now,
        expiresAt: now + (5 * 60 * 1000), // 5分钟过期
      };
      
      await this.database.storeRegistrationState(registrationState);

      // Note: 统一使用新的状态管理系统

      return options;
    } catch (error) {
      throw handleWebAuthnError(error);
    }
  }

  /**
   * 验证注册响应
   */
  async verifyRegistrationResponse(
    response: RegistrationResponseJSON,
    expectedChallenge: string,
    userId: string
  ) {
    try {
      // 获取并验证注册状态
      const registrationState = await this.database.getAndRemoveRegistrationState(userId);
      if (!registrationState || registrationState.challenge !== expectedChallenge) {
        throw WebAuthnError.challengeVerificationFailed(expectedChallenge);
      }

      // 检查状态是否过期
      if (registrationState.expiresAt <= Date.now()) {
        throw WebAuthnError.challengeVerificationFailed(expectedChallenge);
      }

      // Note: 使用新的状态管理，不需要旧的challenges表

      // 测试模式：跳过真实验证，使用模拟数据
      if (this.isTestMode) {
        console.log('🧪 Test mode: Skipping WebAuthn verification, using mock data');
        
        // 生成模拟的设备数据
        const mockCredentialId = Buffer.from(response.id, 'base64');
        const mockPublicKey = Buffer.from('mock_public_key_data_for_testing_32_bytes_length');
        
        // 检查设备是否已存在
        const existingPasskey = await this.database.getPasskeyByCredentialId(mockCredentialId);
        
        if (!existingPasskey) {
          // 确保用户存在于数据库中
          await this.database.createOrUpdateUser(userId, userId, `Test User ${userId.substring(0, 8)}`);
          
          // 保存完整的模拟Passkey到数据库
          await this.database.storePasskey({
            credentialId: mockCredentialId,
            userId,
            credentialPublicKey: mockPublicKey,
            counter: 0,
            transports: response.response.transports || ['internal'],
            aaguid: Buffer.from('mock_aaguid_16_bytes_xx'),
            userHandle: Buffer.from(userId),
            deviceName: 'Mock Test Device',
            backupEligible: false,
            backupState: false,
            uvInitialized: true,
            credentialDeviceType: 'singleDevice'
          });
          
          // Note: 统一使用passkeys表，简化数据结构
          
          console.log(`🧪 Test mode: Mock passkey saved for user ${userId}`);
        } else {
          throw WebAuthnError.deviceAlreadyRegistered(response.id);
        }

        return {
          verified: true,
          userAccount: await this.getUserAccountWithDevices(userId),
        };
      }

      // 生产模式：进行真实验证
      const verification = await verifyRegistrationResponse({
        response,
        expectedChallenge,
        expectedOrigin: this.config.origin,
        expectedRPID: this.config.rpID,
      });

      if (verification.verified && verification.registrationInfo) {
        const { 
          credentialID, 
          credentialPublicKey, 
          counter,
          aaguid,
          credentialBackedUp,
          credentialDeviceType,
          uvInitialized
        } = verification.registrationInfo;
        
        const credentialIdBuffer = Buffer.from(credentialID);
        
        // 检查设备是否已存在
        const existingPasskey = await this.database.getPasskeyByCredentialId(credentialIdBuffer);
        
        if (existingPasskey) {
          throw WebAuthnError.deviceAlreadyRegistered(response.id);
        }

        // 保存完整的Passkey对象
        await this.database.storePasskey({
          credentialId: credentialIdBuffer,
          userId,
          credentialPublicKey: Buffer.from(credentialPublicKey),
          counter,
          transports: response.response.transports || [],
          aaguid: aaguid ? Buffer.from(aaguid) : undefined,
          userHandle: Buffer.from(userId),
          backupEligible: credentialBackedUp || false,
          backupState: credentialBackedUp || false,
          uvInitialized: uvInitialized || false,
          credentialDeviceType: credentialDeviceType || 'singleDevice'
        });

        // Note: 统一使用passkeys表存储完整Passkey信息
      } else {
        throw WebAuthnError.signatureVerificationFailed();
      }

      return {
        verified: verification.verified,
        userAccount: verification.verified ? await this.getUserAccountWithDevices(userId) : undefined,
      };
    } catch (error) {
      throw handleWebAuthnError(error);
    }
  }

  /**
   * 生成认证选项
   */
  async generateAuthenticationOptions(userId?: string) {
    try {
      let allowCredentials: { id: Uint8Array; type: 'public-key'; transports?: any[] }[] = [];

      if (userId) {
        // 检查用户是否存在
        const userAccount = await this.database.getUserAccount(userId);
        if (!userAccount) {
          throw WebAuthnError.userNotFound(userId);
        }

        // 获取用户的Passkey
        const passkeys = await this.database.getUserPasskeys(userId);
        if (passkeys.length === 0) {
          throw WebAuthnError.noDevicesRegistered(userId);
        }

        allowCredentials = passkeys.map(passkey => ({
          id: passkey.credentialId,
          type: 'public-key' as const,
          transports: passkey.transports || [],
        }));
      }

      const userVerification = 'preferred';

      const options = await generateAuthenticationOptions({
        rpID: this.config.rpID,
        allowCredentials,
        userVerification,
      });

      // 存储认证状态
      const now = Date.now();
      const authenticationState: AuthenticationState = {
        challenge: options.challenge,
        userId,
        userVerification,
        createdAt: now,
        expiresAt: now + (5 * 60 * 1000), // 5分钟过期
      };
      
      await this.database.storeAuthenticationState(authenticationState);

      // Note: 统一使用authentication_states表

      return options;
    } catch (error) {
      throw handleWebAuthnError(error);
    }
  }

  /**
   * 验证认证响应
   */
  async verifyAuthenticationResponse(
    response: AuthenticationResponseJSON,
    expectedChallenge: string,
    userId?: string
  ) {
    try {
      // 获取并验证认证状态
      const authenticationState = await this.database.getAndRemoveAuthenticationState(expectedChallenge);
      if (!authenticationState) {
        throw WebAuthnError.challengeVerificationFailed(expectedChallenge);
      }

      // 检查状态是否过期
      if (authenticationState.expiresAt <= Date.now()) {
        throw WebAuthnError.challengeVerificationFailed(expectedChallenge);
      }

      // 验证用户ID是否匹配（如果指定）
      if (userId && authenticationState.userId && authenticationState.userId !== userId) {
        throw WebAuthnError.invalidState('matching user', 'different user');
      }

      // Note: 使用统一的认证状态管理

      // 查找对应的Passkey
      const credentialId = Buffer.from(response.rawId, 'base64');
      const passkey = await this.database.getPasskeyByCredentialId(credentialId);

      if (!passkey) {
        throw WebAuthnError.deviceNotFound(response.rawId);
      }

      // 如果指定了用户ID，验证是否匹配Passkey的用户
      if (userId && passkey.userId !== userId) {
        throw WebAuthnError.deviceNotFound(response.rawId);
      }

      // 测试模式：跳过真实验证
      if (this.isTestMode) {
        console.log('🧪 Test mode: Skipping WebAuthn authentication verification');
        
        // 模拟更新计数器
        await this.database.updatePasskeyCounter(credentialId, passkey.counter + 1);
        await this.database.updateDeviceCounter(credentialId, passkey.counter + 1);

        return {
          verified: true,
          userAccount: await this.getUserAccountWithDevices(passkey.userId),
          authenticationInfo: {
            newCounter: passkey.counter + 1,
            credentialDeviceType: passkey.credentialDeviceType,
            credentialBackedUp: passkey.backupState,
          },
        };
      }

      // 生产模式：进行真实验证
      // 转换为 SimpleWebAuthn 期望的格式
      const simpleWebAuthnDevice = {
        credentialID: passkey.credentialId,
        credentialPublicKey: passkey.credentialPublicKey,
        counter: passkey.counter,
        transports: passkey.transports,
      };

      const verification = await verifyAuthenticationResponse({
        response,
        expectedChallenge,
        expectedOrigin: this.config.origin,
        expectedRPID: this.config.rpID,
        authenticator: simpleWebAuthnDevice,
      });

      if (verification.verified) {
        // 检查计数器回滚攻击
        if (verification.authenticationInfo.newCounter <= passkey.counter) {
          throw WebAuthnError.counterRollback(passkey.counter, verification.authenticationInfo.newCounter);
        }

        // 更新计数器
        await this.database.updatePasskeyCounter(credentialId, verification.authenticationInfo.newCounter);
        await this.database.updateDeviceCounter(credentialId, verification.authenticationInfo.newCounter);
      } else {
        throw WebAuthnError.signatureVerificationFailed();
      }

      return {
        verified: verification.verified,
        userAccount: verification.verified ? await this.getUserAccountWithDevices(passkey.userId) : undefined,
        authenticationInfo: verification.authenticationInfo,
      };
    } catch (error) {
      throw handleWebAuthnError(error);
    }
  }

  /**
   * 获取用户账户
   */
  async getUserAccount(userId: string): Promise<UserAccount | undefined> {
    const dbUser = await this.database.getUserAccount(userId);
    if (!dbUser) return undefined;

    return {
      id: dbUser.userId,
      username: dbUser.username,
      displayName: dbUser.displayName,
      devices: [],
    };
  }

  /**
   * 获取用户账户和设备信息
   */
  async getUserAccountWithDevices(userId: string): Promise<UserAccount | undefined> {
    try {
      const dbUser = await this.database.getUserAccount(userId);
      if (!dbUser) return undefined;

      // 统一使用Passkey数据
      const passkeys = await this.database.getUserPasskeys(userId);
      const devices: AuthenticatorDevice[] = passkeys.map(passkey => ({
        credentialID: passkey.credentialId,
        credentialPublicKey: passkey.credentialPublicKey,
        counter: passkey.counter,
        transports: passkey.transports,
      }));

      return {
        id: dbUser.userId,
        username: dbUser.username,
        displayName: dbUser.displayName,
        devices,
      };
    } catch (error) {
      throw handleWebAuthnError(error);
    }
  }

  /**
   * 获取统计信息
   */
  async getStats() {
    try {
      const stats = await this.database.getUserStats();
      return {
        totalUsers: stats.totalUsers,
        totalDevices: stats.totalDevices,
        activeChallenges: 0, // 由数据库清理过期挑战，这里返回0
        testMode: this.isTestMode,
      };
    } catch (error) {
      throw handleWebAuthnError(error);
    }
  }

  /**
   * 获取用户的完整Passkey信息 - 调试用
   */
  async getUserPasskeys(userId: string) {
    try {
      const userAccount = await this.database.getUserAccount(userId);
      if (!userAccount) {
        throw WebAuthnError.userNotFound(userId);
      }

      const passkeys = await this.database.getUserPasskeys(userId);
      return {
        user: userAccount,
        passkeys: passkeys.map(passkey => ({
          credentialId: passkey.credentialId.toString('base64'),
          counter: passkey.counter,
          transports: passkey.transports,
          deviceName: passkey.deviceName,
          credentialDeviceType: passkey.credentialDeviceType,
          backupEligible: passkey.backupEligible,
          backupState: passkey.backupState,
          uvInitialized: passkey.uvInitialized,
          createdAt: new Date(passkey.createdAt).toISOString(),
          updatedAt: new Date(passkey.updatedAt).toISOString(),
        }))
      };
    } catch (error) {
      throw handleWebAuthnError(error);
    }
  }
}