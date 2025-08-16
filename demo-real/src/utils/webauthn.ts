/**
 * 真实WebAuthn/Passkey工具类
 * 
 * 使用浏览器原生WebAuthn API
 */

import { 
  startRegistration, 
  startAuthentication,
  browserSupportsWebAuthn 
} from '@simplewebauthn/browser'
import type {
  RegistrationResponseJSON,
  AuthenticationResponseJSON,
  PublicKeyCredentialCreationOptionsJSON,
  PublicKeyCredentialRequestOptionsJSON
} from '@simplewebauthn/types'

/**
 * 检查WebAuthn支持
 */
export function checkWebAuthnSupport(): boolean {
  return browserSupportsWebAuthn()
}

/**
 * Passkey注册选项
 */
export interface PasskeyRegistrationOptions {
  userId: string
  userEmail: string
  userName: string
  challenge: string
  rpId?: string
  rpName?: string
}

/**
 * 注册Passkey
 */
export async function registerPasskey(
  options: PasskeyRegistrationOptions
): Promise<RegistrationResponseJSON> {
  if (!checkWebAuthnSupport()) {
    throw new Error('WebAuthn not supported in this browser')
  }

  // 直接使用从服务器返回的选项，SimpleWebAuthn会自动处理格式转换
  const registrationOptions: PublicKeyCredentialCreationOptionsJSON = {
    rp: {
      name: options.rpName || 'AirAccount',
      id: options.rpId || 'localhost'
    },
    user: {
      id: options.userId, // 服务器已经提供了正确格式的ID
      name: options.userEmail,
      displayName: options.userName
    },
    challenge: options.challenge, // 服务器已经提供了正确格式的challenge
    pubKeyCredParams: [
      { alg: -7, type: 'public-key' },  // ES256
      { alg: -257, type: 'public-key' } // RS256
    ],
    timeout: 60000,
    attestation: 'none', // 使用none而不是direct，减少兼容性问题
    authenticatorSelection: {
      authenticatorAttachment: 'platform',
      userVerification: 'preferred', // 使用preferred而不是required
      residentKey: 'preferred'
    },
    excludeCredentials: []
  }

  console.log('🔑 Starting Passkey registration...', {
    userId: options.userId,
    userEmail: options.userEmail,
    rpId: registrationOptions.rp.id,
    challenge: options.challenge.substring(0, 16) + '...'
  })

  try {
    const registrationResponse = await startRegistration(registrationOptions)
    console.log('✅ Passkey registration successful:', registrationResponse)
    return registrationResponse
  } catch (error) {
    console.error('❌ Passkey registration failed:', error)
    throw new Error(`Passkey registration failed: ${(error as Error).message}`)
  }
}

/**
 * Passkey认证选项
 */
export interface PasskeyAuthenticationOptions {
  challenge: string
  rpId?: string
  allowCredentials?: Array<{
    id: string
    type: 'public-key'
  }>
}

/**
 * 使用Passkey认证
 */
export async function authenticateWithPasskey(
  options: PasskeyAuthenticationOptions
): Promise<AuthenticationResponseJSON> {
  if (!checkWebAuthnSupport()) {
    throw new Error('WebAuthn not supported in this browser')
  }

  const authenticationOptions: PublicKeyCredentialRequestOptionsJSON = {
    challenge: options.challenge,
    timeout: 60000,
    rpId: options.rpId || window.location.hostname,
    userVerification: 'required',
    allowCredentials: options.allowCredentials || []
  }

  console.log('🔐 Starting Passkey authentication...', {
    rpId: authenticationOptions.rpId,
    allowCredentials: authenticationOptions.allowCredentials?.length || 0
  })

  try {
    const authenticationResponse = await startAuthentication(authenticationOptions)
    console.log('✅ Passkey authentication successful:', authenticationResponse)
    return authenticationResponse
  } catch (error) {
    console.error('❌ Passkey authentication failed:', error)
    throw new Error(`Passkey authentication failed: ${(error as Error).message}`)
  }
}

/**
 * 检查是否有可用的Passkey
 */
export async function hasAvailablePasskeys(): Promise<boolean> {
  if (!checkWebAuthnSupport()) {
    return false
  }

  try {
    // 检查是否有平台认证器可用
    const available = await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()
    return available
  } catch {
    return false
  }
}

/**
 * 获取WebAuthn支持的详细信息
 */
export async function getWebAuthnInfo(): Promise<{
  supported: boolean
  platform: boolean
  conditional: boolean
  userVerification: boolean
}> {
  const supported = checkWebAuthnSupport()
  
  if (!supported) {
    return {
      supported: false,
      platform: false,
      conditional: false,
      userVerification: false
    }
  }

  try {
    const available = await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()
    const conditional = PublicKeyCredential.isConditionalMediationAvailable ? 
      await PublicKeyCredential.isConditionalMediationAvailable() : false

    return {
      supported: true,
      platform: available,
      conditional,
      userVerification: available
    }
  } catch {
    return {
      supported: true,
      platform: false,
      conditional: false,
      userVerification: false
    }
  }
}