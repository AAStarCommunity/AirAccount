/**
 * 真实AirAccount Demo主界面
 * 
 * 使用真实WebAuthn/Passkey API
 */

import React, { useState, useEffect } from 'react'
import { Shield, Fingerprint, Send, CheckCircle, AlertTriangle, Loader, LogOut } from 'lucide-react'
import { RealAccountCreation } from './components/RealAccountCreation'
import { PasskeyLogin } from './components/PasskeyLogin'
import { checkWebAuthnSupport } from './utils/webauthn'
import axios from 'axios'

const CA_BASE_URL = 'http://localhost:3002'

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

export default function App() {
  const [account, setAccount] = useState<CreateAccountResponse | null>(null)
  const [appStatus, setAppStatus] = useState<'loading' | 'ready' | 'error'>('loading')
  const [error, setError] = useState<string>('')
  const [webauthnSupported, setWebauthnSupported] = useState(false)
  const [mode, setMode] = useState<'login' | 'register'>('login')
  const [isLoggedIn, setIsLoggedIn] = useState(false)

  // 初始化SDK和检查WebAuthn支持
  useEffect(() => {
    initializeApp()
  }, [])

  const initializeApp = async () => {
    try {
      setAppStatus('loading')

      // 检查WebAuthn支持
      const webauthnOk = checkWebAuthnSupport()
      setWebauthnSupported(webauthnOk)
      
      if (!webauthnOk) {
        setAppStatus('error')
        setError('您的浏览器不支持WebAuthn/Passkey。请使用Chrome 67+、Firefox 60+或Safari 14+')
        return
      }

      // 检查CA服务连接
      try {
        await axios.get(`${CA_BASE_URL}/health`)
        console.log('✅ CA服务连接正常')
      } catch (err) {
        throw new Error('无法连接到CA服务，请确保服务正在运行')
      }

      setAppStatus('ready')

    } catch (err) {
      console.error('App initialization failed:', err)
      setAppStatus('error')
      setError(err instanceof Error ? err.message : 'Unknown error')
    }
  }

  const handleAccountCreated = (newAccount: CreateAccountResponse) => {
    setAccount(newAccount)
    setIsLoggedIn(true)
    // 保存到本地存储
    localStorage.setItem('airaccount-real', JSON.stringify(newAccount))
    localStorage.setItem('airaccount-logged-in', 'true')
  }

  const handleLoginSuccess = (authData: any) => {
    // 对于登录，我们只记录登录状态，不覆盖现有账户信息
    setIsLoggedIn(true)
    localStorage.setItem('airaccount-logged-in', 'true')
    console.log('✅ 登录成功:', authData)
  }

  const handleLogout = () => {
    setIsLoggedIn(false)
    localStorage.removeItem('airaccount-logged-in')
  }

  // 从本地存储恢复账户和登录状态
  useEffect(() => {
    const savedAccount = localStorage.getItem('airaccount-real')
    const loggedIn = localStorage.getItem('airaccount-logged-in')
    
    if (savedAccount) {
      try {
        setAccount(JSON.parse(savedAccount))
      } catch (err) {
        console.error('Failed to restore account from localStorage:', err)
      }
    }
    
    if (loggedIn === 'true') {
      setIsLoggedIn(true)
    }
  }, [])

  if (appStatus === 'loading') {
    return (
      <div className="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100 flex items-center justify-center">
        <div className="text-center">
          <Loader className="w-8 h-8 animate-spin mx-auto mb-4 text-blue-600" />
          <p className="text-gray-600">正在初始化真实AirAccount SDK...</p>
          <p className="text-sm text-gray-500 mt-2">检查WebAuthn支持和CA服务连接</p>
        </div>
      </div>
    )
  }

  if (appStatus === 'error') {
    return (
      <div className="min-h-screen bg-gradient-to-br from-red-50 to-pink-100 flex items-center justify-center">
        <div className="text-center max-w-md">
          <AlertTriangle className="w-16 h-16 mx-auto mb-4 text-red-600" />
          <h1 className="text-2xl font-bold text-gray-800 mb-4">初始化失败</h1>
          <p className="text-gray-600 mb-6">{error}</p>
          
          {!webauthnSupported && (
            <div className="p-4 bg-yellow-50 border border-yellow-200 rounded-lg mb-4">
              <h3 className="font-medium text-yellow-800 mb-2">浏览器兼容性</h3>
              <p className="text-sm text-yellow-700">
                真实Passkey功能需要现代浏览器支持：
              </p>
              <ul className="text-xs text-yellow-600 mt-2 space-y-1">
                <li>• Chrome 67+ (推荐)</li>
                <li>• Firefox 60+</li>
                <li>• Safari 14+</li>
                <li>• Edge 18+</li>
              </ul>
            </div>
          )}
          
          <button
            onClick={initializeApp}
            className="px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
          >
            重试
          </button>
        </div>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100">
      {/* 头部 */}
      <header className="bg-white shadow-sm border-b">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-3">
              <Shield className="w-8 h-8 text-blue-600" />
              <div>
                <h1 className="text-2xl font-bold text-gray-900">AirAccount Real</h1>
                <p className="text-sm text-gray-500">真实WebAuthn/Passkey演示</p>
              </div>
            </div>
            
            {account && (
              <div className="flex items-center space-x-4">
                <div className="text-right">
                  <p className="text-sm font-medium text-gray-900">
                    {account.walletResult?.ethereumAddress ? 
                      `${account.walletResult.ethereumAddress.slice(0, 6)}...${account.walletResult.ethereumAddress.slice(-4)}` :
                      'N/A'
                    }
                  </p>
                  <p className="text-xs text-gray-500">真实Passkey保护</p>
                </div>
                <div className="w-8 h-8 bg-green-100 rounded-full flex items-center justify-center">
                  <Fingerprint className="w-5 h-5 text-green-600" />
                </div>
              </div>
            )}
          </div>
        </div>
      </header>

      {/* 主内容 */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {(!account && !isLoggedIn) ? (
          // 登录/注册流程
          <div className="max-w-2xl mx-auto">
            <div className="text-center mb-8">
              <Fingerprint className="w-16 h-16 mx-auto mb-4 text-blue-600" />
              <h2 className="text-3xl font-bold text-gray-900 mb-4">
                {mode === 'login' ? '使用Passkey登录' : '创建真实AirAccount'}
              </h2>
              <p className="text-lg text-gray-600 mb-4">
                {mode === 'login' ? 
                  '使用您已注册的生物识别验证登录' : 
                  '使用您设备的真实生物识别验证'
                }
              </p>
              <div className="text-sm text-gray-500 space-y-1">
                <p>✨ 真实Passkey{mode === 'login' ? '登录' : '注册'}</p>
                <p>🔒 浏览器原生WebAuthn</p>
                <p>⛽ 无Gas费交易</p>
              </div>
              
              {/* 模式切换按钮 */}
              <div className="flex justify-center mt-6">
                <div className="bg-gray-100 p-1 rounded-lg">
                  <button
                    onClick={() => setMode('login')}
                    className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                      mode === 'login' 
                        ? 'bg-white text-blue-600 shadow-sm' 
                        : 'text-gray-600 hover:text-gray-800'
                    }`}
                  >
                    登录
                  </button>
                  <button
                    onClick={() => setMode('register')}
                    className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                      mode === 'register' 
                        ? 'bg-white text-blue-600 shadow-sm' 
                        : 'text-gray-600 hover:text-gray-800'
                    }`}
                  >
                    注册
                  </button>
                </div>
              </div>
            </div>
            
            {mode === 'register' ? (
              <RealAccountCreation 
                baseURL={CA_BASE_URL}
                onAccountCreated={handleAccountCreated}
              />
            ) : (
              <PasskeyLogin
                baseURL={CA_BASE_URL}
                onLoginSuccess={handleLoginSuccess}
                onSwitchToRegister={() => setMode('register')}
              />
            )}
          </div>
        ) : (
          // 主界面
          <div className="max-w-4xl mx-auto space-y-8">
            {/* 成功提示 */}
            <div className="bg-white rounded-xl shadow-lg p-6">
              <div className="flex items-center space-x-3 mb-4">
                <CheckCircle className="w-8 h-8 text-green-600" />
                <div>
                  <h2 className="text-xl font-semibold text-gray-900">账户创建成功！</h2>
                  <p className="text-gray-600">您的AirAccount已使用真实Passkey保护</p>
                </div>
              </div>
              
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4 p-4 bg-gray-50 rounded-lg">
                <div>
                  <span className="text-sm text-gray-600">以太坊地址:</span>
                  <p className="font-mono text-sm text-gray-900 break-all">
                    {account.walletResult?.ethereumAddress || 'N/A'}
                  </p>
                </div>
                <div>
                  <span className="text-sm text-gray-600">钱包ID:</span>
                  <p className="font-mono text-sm text-gray-900">
                    {account.walletResult?.walletId ? 
                      `${account.walletResult.walletId.slice(0, 20)}...` : 
                      'N/A'
                    }
                  </p>
                </div>
                <div>
                  <span className="text-sm text-gray-600">凭证ID:</span>
                  <p className="text-sm text-gray-900">
                    {account.userInstructions?.credentialId ? 
                      `${account.userInstructions.credentialId.slice(0, 20)}...` : 
                      'N/A'
                    }
                  </p>
                </div>
                <div>
                  <span className="text-sm text-gray-600">安全级别:</span>
                  <p className="text-sm text-green-700 font-medium">
                    WebAuthn Protected 🔒
                  </p>
                </div>
              </div>
            </div>

            {/* 功能区域 */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* 余额 */}
              <div className="bg-white rounded-xl shadow-lg p-6">
                <h3 className="text-lg font-semibold text-gray-900 mb-4">账户余额</h3>
                <div className="text-3xl font-bold text-blue-600 mb-2">1.5 ETH</div>
                <p className="text-sm text-gray-500">≈ $3,750 USD</p>
              </div>

              {/* 下一步 */}
              <div className="bg-white rounded-xl shadow-lg p-6">
                <h3 className="text-lg font-semibold text-gray-900 mb-4">下一步开发</h3>
                <ul className="text-sm text-gray-600 space-y-2">
                  <li>• 添加转账功能（需Passkey认证）</li>
                  <li>• 交易历史查询</li>
                  <li>• 真实TEE集成</li>
                  <li>• 区块链交互</li>
                </ul>
              </div>
            </div>

            {/* 重置按钮 */}
            <div className="text-center">
              <button
                onClick={() => {
                  localStorage.removeItem('airaccount-real')
                  setAccount(null)
                }}
                className="px-6 py-3 bg-gray-600 text-white rounded-lg hover:bg-gray-700 transition-colors"
              >
                重置Demo（创建新账户）
              </button>
            </div>
          </div>
        )}
      </main>

      {/* 页脚 */}
      <footer className="bg-white border-t mt-16">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
          <div className="text-center text-gray-500">
            <p className="mb-2">AirAccount Real Demo - 真实WebAuthn/Passkey功能</p>
            <p className="text-sm mb-2">
              真实生物识别 • SQLite存储 • 挑战验证 • 凭证管理
            </p>
            <p className="text-xs">
              架构：DApp → SDK → CA服务 (WebAuthn) → 数据库
            </p>
          </div>
        </div>
      </footer>
    </div>
  )
}