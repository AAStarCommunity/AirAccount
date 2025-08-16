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
  private database: any; // Database instance
  private isTestMode: boolean;

  constructor(config: WebAuthnConfig, database: any, isTestMode: boolean = false) {
    this.config = config;
    this.database = database;
    this.isTestMode = isTestMode;
  }

  /**
   * 生成注册选项
   */
  async generateRegistrationOptions(user: { id: string; username: string; displayName: string }) {
    // 确保用户在数据库中存在
    await this.database.createOrUpdateUser(user.id, user.username, user.displayName);
    
    // 获取用户现有设备（用于排除已注册的设备）
    const existingDevices = await this.database.getUserDevices(user.id);

    const options = await generateRegistrationOptions({
      rpName: this.config.rpName,
      rpID: this.config.rpID,
      userID: user.id,
      userName: user.username,
      userDisplayName: user.displayName,
      attestationType: 'none',
      excludeCredentials: existingDevices.map(device => ({
        id: device.credentialId,
        type: 'public-key' as const,
        transports: device.transports || [],
      })),
      authenticatorSelection: {
        residentKey: 'preferred',
        userVerification: 'preferred',
        authenticatorAttachment: 'platform', // 优先使用平台认证器（Touch ID, Face ID等）
      },
    });

    // 存储 challenge 到数据库
    await this.database.storeChallenge(options.challenge, user.id, 'registration');

    return options;
  }

  /**
   * 验证注册响应
   */
  async verifyRegistrationResponse(
    response: RegistrationResponseJSON,
    expectedChallenge: string,
    userId: string
  ) {
    // 验证 challenge
    const isValidChallenge = await this.database.verifyAndUseChallenge(expectedChallenge, userId);
    if (!isValidChallenge) {
      console.error(`Challenge verification failed for user ${userId}, challenge: ${expectedChallenge.substring(0, 16)}...`);
      throw new Error('Invalid or expired challenge');
    }

    // 测试模式：跳过真实验证，使用模拟数据
    if (this.isTestMode) {
      console.log('🧪 Test mode: Skipping WebAuthn verification, using mock data');
      
      // 生成模拟的设备数据
      const mockCredentialId = Buffer.from(response.id, 'base64');
      const mockPublicKey = Buffer.from('mock_public_key_data_for_testing');
      
      // 检查设备是否已存在
      const existingDevice = await this.database.getDeviceByCredentialId(mockCredentialId);
      
      if (!existingDevice) {
        // 保存模拟设备到数据库
        await this.database.addAuthenticatorDevice({
          userId,
          credentialId: mockCredentialId,
          credentialPublicKey: mockPublicKey,
          counter: 0,
          transports: response.response.transports || ['internal'],
        });
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
      const { credentialID, credentialPublicKey, counter } = verification.registrationInfo;
      
      // 检查设备是否已存在
      const existingDevice = await this.database.getDeviceByCredentialId(Buffer.from(credentialID));
      
      if (!existingDevice) {
        // 保存新设备到数据库
        await this.database.addAuthenticatorDevice({
          userId,
          credentialId: Buffer.from(credentialID),
          credentialPublicKey: Buffer.from(credentialPublicKey),
          counter,
          transports: response.response.transports || [],
        });
      }
    }

    return {
      verified: verification.verified,
      userAccount: verification.verified ? await this.getUserAccountWithDevices(userId) : undefined,
    };
  }

  /**
   * 生成认证选项
   */
  async generateAuthenticationOptions(userId?: string) {
    let allowCredentials: { id: Uint8Array; type: 'public-key'; transports?: any[] }[] = [];

    if (userId) {
      const devices = await this.database.getUserDevices(userId);
      allowCredentials = devices.map(device => ({
        id: device.credentialId,
        type: 'public-key' as const,
        transports: device.transports || [],
      }));
    }

    const options = await generateAuthenticationOptions({
      rpID: this.config.rpID,
      allowCredentials,
      userVerification: 'preferred',
    });

    // 存储 challenge 到数据库
    await this.database.storeChallenge(options.challenge, userId || '', 'authentication');

    return options;
  }

  /**
   * 验证认证响应
   */
  async verifyAuthenticationResponse(
    response: AuthenticationResponseJSON,
    expectedChallenge: string
  ) {
    // 验证 challenge
    const isValidChallenge = await this.database.verifyAndUseChallenge(expectedChallenge);
    if (!isValidChallenge) {
      throw new Error('Invalid or expired challenge');
    }

    // 查找对应的设备
    const credentialId = Buffer.from(response.rawId, 'base64');
    const authenticator = await this.database.getDeviceByCredentialId(credentialId);

    if (!authenticator) {
      throw new Error('Authenticator not found');
    }

    // 测试模式：跳过真实验证
    if (this.isTestMode) {
      console.log('🧪 Test mode: Skipping WebAuthn authentication verification');
      
      // 模拟更新计数器
      await this.database.updateDeviceCounter(credentialId, authenticator.counter + 1);

      return {
        verified: true,
        userAccount: await this.getUserAccountWithDevices(authenticator.userId),
        authenticationInfo: {
          newCounter: authenticator.counter + 1,
          credentialDeviceType: 'singleDevice',
          credentialBackedUp: false,
        },
      };
    }

    // 生产模式：进行真实验证
    // 转换为 SimpleWebAuthn 期望的格式
    const simpleWebAuthnDevice = {
      credentialID: authenticator.credentialId,
      credentialPublicKey: authenticator.credentialPublicKey,
      counter: authenticator.counter,
      transports: authenticator.transports,
    };

    const verification = await verifyAuthenticationResponse({
      response,
      expectedChallenge,
      expectedOrigin: this.config.origin,
      expectedRPID: this.config.rpID,
      authenticator: simpleWebAuthnDevice,
    });

    if (verification.verified) {
      // 更新计数器
      await this.database.updateDeviceCounter(credentialId, verification.authenticationInfo.newCounter);
    }

    return {
      verified: verification.verified,
      userAccount: verification.verified ? await this.getUserAccountWithDevices(authenticator.userId) : undefined,
      authenticationInfo: verification.authenticationInfo,
    };
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
    const dbUser = await this.database.getUserAccount(userId);
    if (!dbUser) return undefined;

    const devices = await this.database.getUserDevices(userId);

    return {
      id: dbUser.userId,
      username: dbUser.username,
      displayName: dbUser.displayName,
      devices: devices.map(device => ({
        credentialID: device.credentialId,
        credentialPublicKey: device.credentialPublicKey,
        counter: device.counter,
        transports: device.transports,
      })),
    };
  }

  /**
   * 获取统计信息
   */
  async getStats() {
    const stats = await this.database.getUserStats();
    return {
      totalUsers: stats.totalUsers,
      totalDevices: stats.totalDevices,
      activeChallenges: 0, // 由数据库清理过期挑战，这里返回0
    };
  }
}