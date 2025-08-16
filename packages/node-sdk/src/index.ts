/**
 * AirAccount SDK - Official JavaScript/TypeScript SDK
 * 
 * Provides Web3 account management with TEE hardware security
 * Architecture: SDK → CA Service → TA → TEE Hardware
 */

import { 
  startRegistration, 
  startAuthentication,
  type RegistrationResponseJSON,
  type AuthenticationResponseJSON
} from '@simplewebauthn/browser';

// Types
export interface AirAccountConfig {
  caBaseUrl: string;
  timeout?: number;
  retries?: number;
}

export interface WebAuthnRegistrationData {
  email: string;
  displayName: string;
}

export interface WebAuthnAuthenticationData {
  email: string;
}

export interface WalletInfo {
  id: number;
  address: string;
  balance?: {
    eth: string;
    tokens?: Record<string, string>;
  };
}

export interface TransferParams {
  to: string;
  amount: string;
  token?: string; // Optional token address, defaults to ETH
}

export interface TransferResult {
  txHash: string;
  status: 'pending' | 'confirmed' | 'failed';
  gasUsed?: string;
}

// Main SDK Class
export class AirAccountSDK {
  private config: Required<AirAccountConfig>;
  private sessionToken?: string;
  private userEmail?: string;

  constructor(config: AirAccountConfig) {
    this.config = {
      timeout: 30000,
      retries: 3,
      ...config
    };
  }

  /**
   * Initialize SDK and check CA service connection
   */
  async initialize(): Promise<void> {
    try {
      const response = await this.request('GET', '/health');
      if (!response.status || response.status !== 'healthy') {
        throw new Error('CA service is not healthy');
      }
    } catch (error) {
      throw new Error(`Failed to initialize SDK: ${error instanceof Error ? error.message : 'Unknown error'}`);
    }
  }

  /**
   * Register new user with WebAuthn/Passkey
   */
  async registerWithWebAuthn(data: WebAuthnRegistrationData): Promise<{
    success: boolean;
    credentialId?: string;
    message?: string;
  }> {
    try {
      // 1. Begin registration - get challenge from CA
      const beginResponse = await this.request('POST', '/api/webauthn/register/begin', {
        email: data.email,
        displayName: data.displayName
      });

      if (!beginResponse.options) {
        throw new Error('Failed to get WebAuthn options from CA service');
      }

      // 2. Use browser WebAuthn API to create credential
      const registrationResponse = await startRegistration(beginResponse.options);

      // 3. Complete registration with CA
      const completeResponse = await this.request('POST', '/api/webauthn/register/complete', {
        email: data.email,
        response: registrationResponse
      });

      if (completeResponse.success) {
        this.userEmail = data.email;
        this.sessionToken = completeResponse.sessionToken;
      }

      return {
        success: completeResponse.success,
        credentialId: registrationResponse.id,
        message: completeResponse.message
      };
    } catch (error) {
      return {
        success: false,
        message: error instanceof Error ? error.message : 'Registration failed'
      };
    }
  }

  /**
   * Authenticate user with WebAuthn/Passkey
   */
  async authenticateWithWebAuthn(data: WebAuthnAuthenticationData): Promise<{
    success: boolean;
    message?: string;
  }> {
    try {
      // 1. Begin authentication - get challenge from CA
      const beginResponse = await this.request('POST', '/api/webauthn/authenticate/begin', {
        email: data.email
      });

      if (!beginResponse.options) {
        throw new Error('Failed to get WebAuthn authentication options');
      }

      // 2. Use browser WebAuthn API to authenticate
      const authResponse = await startAuthentication(beginResponse.options);

      // 3. Complete authentication with CA
      const completeResponse = await this.request('POST', '/api/webauthn/authenticate/complete', {
        email: data.email,
        response: authResponse
      });

      if (completeResponse.success) {
        this.userEmail = data.email;
        this.sessionToken = completeResponse.sessionToken;
      }

      return {
        success: completeResponse.success,
        message: completeResponse.message
      };
    } catch (error) {
      return {
        success: false,
        message: error instanceof Error ? error.message : 'Authentication failed'
      };
    }
  }

  /**
   * Create new wallet account in TEE
   */
  async createAccount(): Promise<WalletInfo> {
    this.requireAuthentication();

    const response = await this.request('POST', '/api/account/create', {
      email: this.userEmail
    }, true);

    if (!response.success) {
      throw new Error(`Failed to create account: ${response.message || 'Unknown error'}`);
    }

    return {
      id: response.walletId,
      address: response.address
    };
  }

  /**
   * Get wallet balance
   */
  async getBalance(walletId?: number): Promise<WalletInfo> {
    this.requireAuthentication();

    const response = await this.request('POST', '/api/account/balance', {
      email: this.userEmail,
      walletId: walletId
    }, true);

    if (!response.success) {
      throw new Error(`Failed to get balance: ${response.message || 'Unknown error'}`);
    }

    return {
      id: response.walletId,
      address: response.address,
      balance: {
        eth: response.balance?.eth || response.balance || '0',
        tokens: response.balance?.tokens
      }
    };
  }

  /**
   * Transfer funds
   */
  async transfer(params: TransferParams, walletId?: number): Promise<TransferResult> {
    this.requireAuthentication();

    const response = await this.request('POST', '/api/transaction/transfer', {
      email: this.userEmail,
      walletId: walletId,
      to: params.to,
      amount: params.amount,
      token: params.token
    }, true);

    if (!response.success) {
      throw new Error(`Transfer failed: ${response.message || 'Unknown error'}`);
    }

    return {
      txHash: response.txHash,
      status: 'pending', // CA should provide real status
      gasUsed: response.gasUsed
    };
  }

  /**
   * List all wallets for current user
   */
  async listWallets(): Promise<WalletInfo[]> {
    this.requireAuthentication();

    const response = await this.request('GET', `/api/wallet/list?email=${encodeURIComponent(this.userEmail!)}`, null, true);

    if (!response.success) {
      throw new Error(`Failed to list wallets: ${response.message || 'Unknown error'}`);
    }

    return response.wallets?.map((wallet: any) => ({
      id: wallet.id,
      address: wallet.address,
      balance: wallet.balance ? {
        eth: wallet.balance.eth || wallet.balance,
        tokens: wallet.balance.tokens
      } : undefined
    })) || [];
  }

  /**
   * Get current user email
   */
  getCurrentUser(): string | undefined {
    return this.userEmail;
  }

  /**
   * Check if user is authenticated
   */
  isAuthenticated(): boolean {
    return !!(this.userEmail && this.sessionToken);
  }

  /**
   * Logout current user
   */
  logout(): void {
    this.userEmail = undefined;
    this.sessionToken = undefined;
  }

  // Private helper methods
  private requireAuthentication(): void {
    if (!this.isAuthenticated()) {
      throw new Error('User not authenticated. Please call registerWithWebAuthn() or authenticateWithWebAuthn() first.');
    }
  }

  private async request(
    method: 'GET' | 'POST' | 'PUT' | 'DELETE',
    path: string,
    body?: any,
    requireAuth = false
  ): Promise<any> {
    const url = `${this.config.caBaseUrl}${path}`;
    
    const headers: Record<string, string> = {
      'Content-Type': 'application/json'
    };

    if (requireAuth && this.sessionToken) {
      headers.Authorization = `Bearer ${this.sessionToken}`;
    }

    let lastError: Error;

    for (let attempt = 0; attempt < this.config.retries; attempt++) {
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), this.config.timeout);

        const response = await fetch(url, {
          method,
          headers,
          body: body ? JSON.stringify(body) : undefined,
          signal: controller.signal
        });

        clearTimeout(timeoutId);

        if (!response.ok) {
          throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }

        const data = await response.json();
        return data;
      } catch (error) {
        lastError = error instanceof Error ? error : new Error('Request failed');
        
        if (attempt < this.config.retries - 1) {
          // Wait before retry (exponential backoff)
          await new Promise(resolve => setTimeout(resolve, Math.pow(2, attempt) * 1000));
        }
      }
    }

    throw lastError!;
  }
}

// Convenience function for quick setup
export function createAirAccountSDK(config: AirAccountConfig): AirAccountSDK {
  return new AirAccountSDK(config);
}

// Export types for TypeScript users
export type {
  AirAccountConfig,
  WebAuthnRegistrationData,
  WebAuthnAuthenticationData,
  WalletInfo,
  TransferParams,
  TransferResult
};