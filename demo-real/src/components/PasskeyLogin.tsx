/**
 * Passkey登录组件
 * 
 * 实现传统passkey登录流程
 */

import React, { useState } from 'react'
import { Fingerprint, Loader, AlertCircle, Mail, LogIn } from 'lucide-react'
import { authenticateWithPasskey, checkWebAuthnSupport } from '../utils/webauthn'
import axios from 'axios'

interface AuthenticationResponse {
  success: boolean
  sessionId?: string
  userAccount?: {
    email: string
    userId: string
    deviceCount: number
  }
  sessionInfo?: {
    expiresIn: number
    message: string
  }
}

interface Props {
  baseURL?: string
  onLoginSuccess: (auth: AuthenticationResponse) => void
  onSwitchToRegister: () => void
}

export function PasskeyLogin({ 
  baseURL = 'http://localhost:3002', 
  onLoginSuccess,
  onSwitchToRegister 
}: Props) {
  const [step, setStep] = useState<'email' | 'authenticating'>('email')
  const [email, setEmail] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const handleEmailSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!email.trim()) {
      setError('请输入有效的邮箱地址')
      return
    }
    setError('')
    handlePasskeyAuthentication()
  }

  const handlePasskeyAuthentication = async () => {
    try {
      setLoading(true)
      setError('')
      setStep('authenticating')

      console.log('🔐 开始Passkey登录流程...')

      // 1. 从CA服务获取认证挑战
      const challengeResponse = await axios.post(`${baseURL}/api/webauthn/authenticate/begin`, {
        email
      })

      const { options } = challengeResponse.data
      console.log('📋 获取到认证挑战:', { challenge: options.challenge })

      // 2. 使用真实WebAuthn API进行认证
      const authenticationResult = await authenticateWithPasskey({
        challenge: options.challenge,
        rpId: options.rpId,
        allowCredentials: options.allowCredentials
      })

      console.log('🔑 Passkey认证成功:', authenticationResult)

      // 3. 将认证结果发送到CA服务完成登录
      const loginResponse = await axios.post(`${baseURL}/api/webauthn/authenticate/finish`, {
        email,
        authenticationResponse: authenticationResult,
        challenge: options.challenge
      })

      const authData = loginResponse.data
      console.log('✅ 登录成功:', authData)

      onLoginSuccess(authData)

    } catch (err) {
      console.error('❌ Passkey登录失败:', err)
      setError(err instanceof Error ? err.message : '登录失败')
      setStep('email')
    } finally {
      setLoading(false)
    }
  }

  if (step === 'authenticating') {
    return (
      <div className="bg-white rounded-xl shadow-lg p-8">
        <div className="text-center">
          <div className="relative mb-6">
            <Loader className="w-16 h-16 animate-spin mx-auto text-blue-600" />
            <Fingerprint className="w-8 h-8 absolute top-4 left-1/2 transform -translate-x-1/2 text-blue-800" />
          </div>
          
          <h3 className="text-xl font-semibold text-gray-900 mb-4">
            正在验证您的身份
          </h3>
          
          <div className="space-y-3 text-sm text-gray-600">
            <div className="flex items-center justify-center space-x-2">
              <Loader className="w-4 h-4 animate-spin text-blue-600" />
              <span>等待生物识别验证...</span>
            </div>
            <p className="text-xs text-gray-500">
              请在您的设备上使用指纹、面容ID或其他生物识别方式
            </p>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="bg-white rounded-xl shadow-lg p-8">
      <div className="text-center mb-8">
        <LogIn className="w-16 h-16 mx-auto mb-4 text-blue-600" />
        <h3 className="text-xl font-semibold text-gray-900 mb-2">
          使用Passkey登录
        </h3>
        <p className="text-gray-600">
          输入您注册时使用的邮箱地址
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
          disabled={loading}
          className="w-full py-4 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center justify-center space-x-2"
        >
          {loading ? (
            <Loader className="w-5 h-5 animate-spin" />
          ) : (
            <>
              <Fingerprint className="w-5 h-5" />
              <span>使用Passkey登录</span>
            </>
          )}
        </button>

        <button
          type="button"
          onClick={onSwitchToRegister}
          className="w-full py-3 text-gray-600 hover:text-gray-800 transition-colors"
        >
          还没有账户？点击注册
        </button>
      </form>
    </div>
  )
}