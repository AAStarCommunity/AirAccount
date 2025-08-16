/**
 * 真实账户创建组件
 * 
 * 使用真实的WebAuthn/Passkey API
 */

import React, { useState, useEffect } from 'react'
import { Fingerprint, Loader, CheckCircle, AlertCircle, Mail, Shield } from 'lucide-react'
import { registerPasskey, checkWebAuthnSupport, getWebAuthnInfo } from '../utils/webauthn'
import axios from 'axios'

interface CreateAccountResponse {
  success: boolean
  walletResult?: {
    walletId: string
    ethereumAddress: string
  }
  userInstructions?: {
    credentialId: string
    message: string
    recoveryInfo: {
      email: string
      credentialId: string
      walletId: string
      ethereumAddress: string
    }
  }
}

interface Props {
  baseURL?: string
  onAccountCreated: (account: CreateAccountResponse) => void
}

export function RealAccountCreation({ baseURL = 'http://localhost:3002', onAccountCreated }: Props) {
  const [step, setStep] = useState<'check' | 'email' | 'passkey' | 'creating'>('check')
  const [email, setEmail] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [webauthnInfo, setWebauthnInfo] = useState<any>(null)

  useEffect(() => {
    checkWebAuthnCapabilities()
  }, [])

  const checkWebAuthnCapabilities = async () => {
    try {
      const info = await getWebAuthnInfo()
      setWebauthnInfo(info)
      
      if (!info.supported) {
        setError('您的浏览器不支持WebAuthn/Passkey功能')
        return
      }
      
      if (!info.platform) {
        setError('您的设备不支持平台认证器（指纹/面容）')
        return
      }
      
      setStep('email')
    } catch (err) {
      console.error('WebAuthn check failed:', err)
      setError('检查WebAuthn支持时出错')
    }
  }

  const handleEmailSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!email.trim()) {
      setError('请输入有效的邮箱地址')
      return
    }
    setError('')
    setStep('passkey')
  }

  const handlePasskeyRegistration = async () => {
    try {
      setLoading(true)
      setError('')

      console.log('🚀 开始真实Passkey注册流程...')

      // 1. 从CA服务获取注册挑战
      const challengeResponse = await axios.post(`${baseURL}/api/webauthn/register/begin`, {
        email,
        displayName: email.split('@')[0]
      })

      const { options, sessionId } = challengeResponse.data
      console.log('📋 获取到注册挑战:', { challenge: options.challenge, sessionId })
      console.log('📋 获取到注册挑战:', { challenge, userId })

      // 2. 使用真实WebAuthn API注册Passkey
      const registrationResult = await registerPasskey({
        userId: options.user.id,
        userEmail: email,
        userName: email.split('@')[0],
        challenge: options.challenge,
        rpName: options.rp.name,
        rpId: options.rp.id
      })

      console.log('🔑 Passkey注册成功:', registrationResult)

      // 3. 将注册结果发送到CA服务完成账户创建
      setStep('creating')
      
      const createAccountResponse = await axios.post(`${baseURL}/api/webauthn/register/finish`, {
        email,
        registrationResponse: registrationResult,
        challenge: options.challenge
      })

      const accountData = createAccountResponse.data
      console.log('✅ 账户创建成功:', accountData)

      onAccountCreated(accountData)

    } catch (err) {
      console.error('❌ Passkey注册失败:', err)
      setError(err instanceof Error ? err.message : '注册失败')
      setStep('passkey')
    } finally {
      setLoading(false)
    }
  }

  if (step === 'creating') {
    return (
      <div className="bg-white rounded-xl shadow-lg p-8">
        <div className="text-center">
          <div className="relative mb-6">
            <Loader className="w-16 h-16 animate-spin mx-auto text-blue-600" />
            <Fingerprint className="w-8 h-8 absolute top-4 left-1/2 transform -translate-x-1/2 text-blue-800" />
          </div>
          
          <h3 className="text-xl font-semibold text-gray-900 mb-4">
            正在创建您的AirAccount
          </h3>
          
          <div className="space-y-3 text-sm text-gray-600">
            <div className="flex items-center justify-center space-x-2">
              <CheckCircle className="w-4 h-4 text-green-600" />
              <span>Passkey注册完成</span>
            </div>
            <div className="flex items-center justify-center space-x-2">
              <Loader className="w-4 h-4 animate-spin text-blue-600" />
              <span>TEE生成密钥中...</span>
            </div>
            <div className="flex items-center justify-center space-x-2">
              <Loader className="w-4 h-4 animate-spin text-blue-600" />
              <span>部署智能合约钱包...</span>
            </div>
          </div>
        </div>
      </div>
    )
  }

  if (step === 'passkey') {
    return (
      <div className="bg-white rounded-xl shadow-lg p-8">
        <div className="text-center mb-8">
          <Fingerprint className="w-16 h-16 mx-auto mb-4 text-blue-600" />
          <h3 className="text-xl font-semibold text-gray-900 mb-2">
            设置生物识别验证
          </h3>
          <p className="text-gray-600 mb-4">
            使用您设备的指纹或面容ID来保护账户
          </p>
          
          {/* WebAuthn 支持信息 */}
          {webauthnInfo && (
            <div className="p-3 bg-green-50 border border-green-200 rounded-lg text-sm text-green-800 mb-4">
              <div className="flex items-center justify-center space-x-2 mb-2">
                <Shield className="w-4 h-4" />
                <span className="font-medium">设备安全验证</span>
              </div>
              <div className="space-y-1">
                <p>✅ WebAuthn支持: {webauthnInfo.supported ? '是' : '否'}</p>
                <p>✅ 平台认证器: {webauthnInfo.platform ? '可用' : '不可用'}</p>
                <p>✅ 用户验证: {webauthnInfo.userVerification ? '支持' : '不支持'}</p>
              </div>
            </div>
          )}
        </div>

        {error && (
          <div className="mb-6 p-4 bg-red-50 border border-red-200 rounded-lg flex items-center space-x-2">
            <AlertCircle className="w-5 h-5 text-red-600" />
            <span className="text-red-700">{error}</span>
          </div>
        )}

        <div className="space-y-4">
          <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg">
            <p className="text-sm text-blue-800 mb-2">
              <strong>邮箱:</strong> {email}
            </p>
            <p className="text-xs text-blue-600">
              点击下方按钮将触发您设备的生物识别验证
            </p>
          </div>

          <button
            onClick={handlePasskeyRegistration}
            disabled={loading}
            className="w-full py-4 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center justify-center space-x-2"
          >
            {loading ? (
              <Loader className="w-5 h-5 animate-spin" />
            ) : (
              <>
                <Fingerprint className="w-5 h-5" />
                <span>注册生物识别</span>
              </>
            )}
          </button>

          <button
            onClick={() => setStep('email')}
            className="w-full py-3 text-gray-600 hover:text-gray-800 transition-colors"
          >
            返回修改邮箱
          </button>
        </div>
      </div>
    )
  }

  if (step === 'email') {
    return (
      <div className="bg-white rounded-xl shadow-lg p-8">
        <div className="text-center mb-8">
          <Mail className="w-16 h-16 mx-auto mb-4 text-blue-600" />
          <h3 className="text-xl font-semibold text-gray-900 mb-2">
            输入您的邮箱
          </h3>
          <p className="text-gray-600">
            我们将使用您的邮箱创建AirAccount
          </p>
        </div>

        <form onSubmit={handleEmailSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              邮箱地址
            </label>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full px-4 py-3 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              placeholder="your@example.com"
              required
            />
          </div>

          {error && (
            <div className="p-4 bg-red-50 border border-red-200 rounded-lg flex items-center space-x-2">
              <AlertCircle className="w-5 h-5 text-red-600" />
              <span className="text-red-700">{error}</span>
            </div>
          )}

          <button
            type="submit"
            className="w-full py-4 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            继续
          </button>
        </form>
      </div>
    )
  }

  // step === 'check'
  return (
    <div className="bg-white rounded-xl shadow-lg p-8">
      <div className="text-center">
        <Loader className="w-16 h-16 animate-spin mx-auto mb-4 text-blue-600" />
        <h3 className="text-xl font-semibold text-gray-900 mb-2">
          检查设备兼容性
        </h3>
        <p className="text-gray-600">
          正在检查您的设备是否支持生物识别验证...
        </p>
        
        {error && (
          <div className="mt-6 p-4 bg-red-50 border border-red-200 rounded-lg">
            <div className="flex items-center space-x-2 mb-2">
              <AlertCircle className="w-5 h-5 text-red-600" />
              <span className="font-medium text-red-700">设备不兼容</span>
            </div>
            <p className="text-sm text-red-600">{error}</p>
            <p className="text-xs text-red-500 mt-2">
              建议使用支持WebAuthn的现代浏览器，如Chrome 67+、Firefox 60+、Safari 14+
            </p>
          </div>
        )}
      </div>
    </div>
  )
}