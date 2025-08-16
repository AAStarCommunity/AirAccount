/**
 * WebAuthn管理器 - 基于passkey-demo最佳实践
 * 
 * 参考资料：
 * - https://github.com/oceans404/passkey-demo
 * - https://github.com/mingder78/all-about-abstract-account
 */

import { 
  startRegistration, 
  startAuthentication,
  type PublicKeyCredentialCreationOptionsJSON,
  type PublicKeyCredentialRequestOptionsJSON,
  type RegistrationResponseJSON,
  type AuthenticationResponseJSON
} from '@simplewebauthn/browser';

export interface WebAuthnUser {
  id: string;
  email: string;
  displayName: string;
}

export interface WebAuthnCredential {
  credentialId: string;
  publicKey: string;
  counter: number;
  createdAt: Date;
  lastUsed: Date;
}

export interface PasskeyRegistrationOptions {
  user: WebAuthnUser;
  challenge?: string;
  excludeCredentials?: string[];
  authenticatorAttachment?: 'platform' | 'cross-platform';
  userVerification?: 'required' | 'preferred' | 'discouraged';
}

export interface PasskeyAuthenticationOptions {
  challenge?: string;
  allowCredentials?: string[];
  userVerification?: 'required' | 'preferred' | 'discouraged';
}

export interface WebAuthnConfig {
  rpName: string;
  rpId: string;
  origin: string;
  apiBaseUrl: string;
  timeout?: number;
}

/**
 * WebAuthn管理器
 * 提供完整的Passkey注册和认证功能
 */
export class WebAuthnManager {
  private config: WebAuthnConfig;
  private currentUser: WebAuthnUser | null = null;
  private sessionToken: string | null = null;

  constructor(config: WebAuthnConfig) {
    this.config = {
      timeout: 60000, // 默认60秒超时
      ...config
    };
  }

  /**
   * 检查浏览器是否支持WebAuthn
   */
  static isSupported(): boolean {
    return typeof window !== 'undefined' && 
           window.PublicKeyCredential !== undefined &&
           typeof window.PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable === 'function';
  }

  /**
   * 检查平台认证器是否可用（Touch ID, Face ID等）
   */
  static async isPlatformAuthenticatorAvailable(): Promise<boolean> {
    if (!this.isSupported()) {
      return false;
    }

    try {
      return await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
    } catch {
      return false;
    }
  }

  /**
   * 注册新的Passkey
   * 
   * @param user 用户信息
   * @param options 注册选项
   * @returns 注册结果和会话信息
   */
  async registerPasskey(
    user: WebAuthnUser, 
    options: PasskeyRegistrationOptions = {}
  ): Promise<{
    success: boolean;
    sessionToken?: string;
    credential?: WebAuthnCredential;
    walletInfo?: any;
    error?: string;
  }> {
    try {
      // 步骤1: 从服务器获取注册选项
      const registrationOptions = await this.getRegistrationOptions(user, options);
      
      console.log('🔐 Starting passkey registration for:', user.email);
      
      // 步骤2: 调用浏览器WebAuthn API创建凭证
      const registrationResponse = await startRegistration(registrationOptions);
      
      console.log('✅ Passkey created successfully');
      
      // 步骤3: 服务器验证并完成注册
      const verificationResult = await this.verifyRegistration(
        user.email,
        registrationResponse,
        registrationOptions.challenge
      );
      
      if (verificationResult.success) {
        this.currentUser = user;
        this.sessionToken = verificationResult.sessionToken;
        
        return {
          success: true,
          sessionToken: verificationResult.sessionToken,
          credential: verificationResult.credential,
          walletInfo: verificationResult.walletResult
        };
      } else {
        return {
          success: false,
          error: verificationResult.error || 'Registration verification failed'
        };
      }
      
    } catch (error: any) {
      console.error('Passkey registration failed:', error);
      
      // 处理用户取消操作
      if (error.name === 'NotAllowedError') {
        return {
          success: false,
          error: 'User cancelled the registration process'
        };
      }
      
      // 处理不支持的认证器
      if (error.name === 'NotSupportedError') {
        return {
          success: false,
          error: 'This device does not support passkeys'
        };
      }
      
      return {
        success: false,
        error: error.message || 'Unknown registration error'
      };
    }
  }

  /**
   * 使用Passkey进行认证
   * 
   * @param email 用户邮箱（可选，支持无密码模式）
   * @param options 认证选项
   * @returns 认证结果和会话信息
   */
  async authenticateWithPasskey(
    email?: string,
    options: PasskeyAuthenticationOptions = {}
  ): Promise<{
    success: boolean;
    sessionToken?: string;
    userAccount?: any;
    error?: string;
  }> {
    try {
      // 步骤1: 从服务器获取认证选项
      const authOptions = await this.getAuthenticationOptions(email, options);
      
      console.log(`🔓 Starting passkey authentication${email ? ' for: ' + email : ' (passwordless)'}`);
      
      // 步骤2: 调用浏览器WebAuthn API进行认证
      const authResponse = await startAuthentication(authOptions);
      
      console.log('✅ Passkey authentication successful');
      
      // 步骤3: 服务器验证认证响应
      const verificationResult = await this.verifyAuthentication(
        email || '',
        authResponse,
        authOptions.challenge
      );
      
      if (verificationResult.success) {
        this.sessionToken = verificationResult.sessionToken;
        this.currentUser = verificationResult.userAccount;
        
        return {
          success: true,
          sessionToken: verificationResult.sessionToken,
          userAccount: verificationResult.userAccount
        };
      } else {
        return {
          success: false,
          error: verificationResult.error || 'Authentication verification failed'
        };
      }
      
    } catch (error: any) {
      console.error('Passkey authentication failed:', error);
      
      // 处理用户取消操作
      if (error.name === 'NotAllowedError') {
        return {
          success: false,
          error: 'User cancelled the authentication process'
        };
      }
      
      // 处理凭证未找到
      if (error.name === 'InvalidStateError') {
        return {
          success: false,
          error: 'No passkey found for this device'
        };
      }
      
      return {
        success: false,
        error: error.message || 'Unknown authentication error'
      };
    }
  }

  /**
   * 登出当前会话
   */
  async logout(): Promise<void> {
    if (this.sessionToken) {
      try {
        await fetch(`${this.config.apiBaseUrl}/auth/logout`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            sessionId: this.sessionToken
          })
        });
      } catch (error) {
        console.warn('Logout request failed:', error);
      }
    }
    
    this.currentUser = null;
    this.sessionToken = null;
  }

  /**
   * 获取当前用户信息
   */
  getCurrentUser(): WebAuthnUser | null {
    return this.currentUser;
  }

  /**
   * 获取当前会话令牌
   */
  getSessionToken(): string | null {
    return this.sessionToken;
  }

  /**
   * 检查是否已认证
   */
  isAuthenticated(): boolean {
    return this.sessionToken !== null && this.currentUser !== null;
  }

  // === 私有方法 ===

  /**
   * 从服务器获取注册选项
   */
  private async getRegistrationOptions(
    user: WebAuthnUser,
    options: PasskeyRegistrationOptions
  ): Promise<PublicKeyCredentialCreationOptionsJSON> {
    const response = await fetch(`${this.config.apiBaseUrl}/webauthn/register/begin`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        email: user.email,
        displayName: user.displayName,
        authenticatorAttachment: options.authenticatorAttachment || 'platform',
        userVerification: options.userVerification || 'preferred',
        excludeCredentials: options.excludeCredentials || []
      })
    });

    if (!response.ok) {
      throw new Error(`Registration options request failed: ${response.statusText}`);
    }

    const data = await response.json();
    
    if (!data.success) {
      throw new Error(data.error || 'Failed to get registration options');
    }

    return data.options;
  }

  /**
   * 验证注册响应
   */
  private async verifyRegistration(
    email: string,
    registrationResponse: RegistrationResponseJSON,
    challenge: string
  ): Promise<any> {
    const response = await fetch(`${this.config.apiBaseUrl}/webauthn/register/finish`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        email,
        registrationResponse,
        challenge
      })
    });

    if (!response.ok) {
      throw new Error(`Registration verification failed: ${response.statusText}`);
    }

    return await response.json();
  }

  /**
   * 从服务器获取认证选项
   */
  private async getAuthenticationOptions(
    email?: string,
    options: PasskeyAuthenticationOptions = {}
  ): Promise<PublicKeyCredentialRequestOptionsJSON> {
    const response = await fetch(`${this.config.apiBaseUrl}/webauthn/authenticate/begin`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        email,
        userVerification: options.userVerification || 'preferred',
        allowCredentials: options.allowCredentials || []
      })
    });

    if (!response.ok) {
      throw new Error(`Authentication options request failed: ${response.statusText}`);
    }

    const data = await response.json();
    
    if (!data.success) {
      throw new Error(data.error || 'Failed to get authentication options');
    }

    return data.options;
  }

  /**
   * 验证认证响应
   */
  private async verifyAuthentication(
    email: string,
    authResponse: AuthenticationResponseJSON,
    challenge: string
  ): Promise<any> {
    const response = await fetch(`${this.config.apiBaseUrl}/webauthn/authenticate/finish`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        email,
        authenticationResponse: authResponse,
        challenge
      })
    });

    if (!response.ok) {
      throw new Error(`Authentication verification failed: ${response.statusText}`);
    }

    return await response.json();
  }
}