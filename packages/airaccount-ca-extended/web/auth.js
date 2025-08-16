/**
 * AirAccount Web2 Authentication Frontend
 * 支持Email验证、OAuth2登录和Passkey注册的完整认证流程
 * 
 * 功能：
 * 1. Email验证 - 发送验证码并验证
 * 2. OAuth2登录 - Google、GitHub等第三方登录
 * 3. Passkey注册 - WebAuthn设备注册
 * 4. 钱包创建 - 与TEE后端交互创建钱包
 */

class AirAccountAuth {
    constructor(config = {}) {
        this.apiBaseUrl = config.apiBaseUrl || 'http://localhost:3001';
        this.currentStep = 'email'; // email, oauth, passkey, complete
        this.userInfo = {};
        this.sessionId = null;
        
        this.initializeUI();
        this.bindEvents();
    }

    initializeUI() {
        // 创建主容器
        const container = document.createElement('div');
        container.className = 'airaccount-auth-container';
        container.innerHTML = `
            <div class="auth-card">
                <div class="auth-header">
                    <h1>🔐 AirAccount</h1>
                    <p>安全的TEE驱动Web3钱包</p>
                </div>
                
                <!-- Email验证步骤 -->
                <div id="email-step" class="auth-step active">
                    <h2>邮箱验证</h2>
                    <form id="email-form">
                        <div class="input-group">
                            <label for="email">邮箱地址</label>
                            <input type="email" id="email" required placeholder="your@email.com">
                        </div>
                        <button type="submit" class="btn-primary">发送验证码</button>
                    </form>
                    
                    <div id="verification-form" class="hidden">
                        <div class="input-group">
                            <label for="verification-code">验证码</label>
                            <input type="text" id="verification-code" maxlength="6" placeholder="6位验证码">
                        </div>
                        <button id="verify-code-btn" class="btn-primary">验证</button>
                    </div>
                    
                    <div class="divider">或</div>
                    
                    <div class="oauth-buttons">
                        <button id="google-login" class="btn-oauth google">
                            <img src="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTgiIGhlaWdodD0iMTgiIHZpZXdCb3g9IjAgMCAxOCAxOCI+PC9zdmc+" alt="Google">
                            使用Google登录
                        </button>
                        <button id="github-login" class="btn-oauth github">
                            <img src="data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTgiIGhlaWdodD0iMTgiIHZpZXdCb3g9IjAgMCAxOCAxOCI+PC9zdmc+" alt="GitHub">
                            使用GitHub登录
                        </button>
                    </div>
                </div>

                <!-- Passkey注册步骤 -->
                <div id="passkey-step" class="auth-step">
                    <h2>设置Passkey</h2>
                    <p>使用您的设备生物识别（Face ID、指纹等）创建安全的Passkey</p>
                    
                    <div class="user-info">
                        <p><strong>用户:</strong> <span id="user-email"></span></p>
                        <p><strong>来源:</strong> <span id="user-source"></span></p>
                    </div>
                    
                    <button id="create-passkey-btn" class="btn-primary">
                        <span class="icon">🔑</span>
                        创建Passkey
                    </button>
                    
                    <div class="passkey-help">
                        <h4>什么是Passkey？</h4>
                        <ul>
                            <li>使用设备生物识别（Face ID、指纹）</li>
                            <li>私钥安全存储在您的设备中</li>
                            <li>无需记住密码</li>
                            <li>抗钓鱼攻击</li>
                        </ul>
                    </div>
                </div>

                <!-- 钱包创建步骤 -->
                <div id="wallet-step" class="auth-step">
                    <h2>创建钱包</h2>
                    <p>正在在TEE环境中创建您的安全钱包...</p>
                    
                    <div class="loading-spinner">
                        <div class="spinner"></div>
                        <p>连接TEE环境...</p>
                    </div>
                    
                    <div id="wallet-result" class="hidden">
                        <div class="success-message">
                            <h3>✅ 钱包创建成功！</h3>
                            <div class="wallet-info">
                                <p><strong>钱包ID:</strong> <span id="wallet-id"></span></p>
                                <p><strong>以太坊地址:</strong> <span id="ethereum-address"></span></p>
                                <p><strong>TEE设备ID:</strong> <span id="tee-device-id"></span></p>
                            </div>
                            
                            <div class="recovery-info">
                                <h4>🔒 重要恢复信息</h4>
                                <p>请保存以下信息用于钱包恢复：</p>
                                <div class="recovery-data">
                                    <pre id="recovery-json"></pre>
                                </div>
                                <button id="download-recovery" class="btn-secondary">下载恢复信息</button>
                            </div>
                            
                            <button id="continue-to-wallet" class="btn-primary">进入钱包</button>
                        </div>
                    </div>
                </div>

                <!-- 状态和错误显示 -->
                <div id="status-message" class="status-message hidden"></div>
                <div id="error-message" class="error-message hidden"></div>
            </div>
        `;
        
        document.body.appendChild(container);
        this.addStyles();
    }

    addStyles() {
        const style = document.createElement('style');
        style.textContent = `
            .airaccount-auth-container {
                display: flex;
                justify-content: center;
                align-items: center;
                min-height: 100vh;
                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            }
            
            .auth-card {
                background: white;
                padding: 2rem;
                border-radius: 12px;
                box-shadow: 0 8px 32px rgba(0, 0, 0, 0.1);
                width: 100%;
                max-width: 400px;
                min-height: 500px;
            }
            
            .auth-header {
                text-align: center;
                margin-bottom: 2rem;
            }
            
            .auth-header h1 {
                margin: 0;
                color: #333;
                font-size: 2rem;
            }
            
            .auth-header p {
                margin: 0.5rem 0 0 0;
                color: #666;
                font-size: 0.9rem;
            }
            
            .auth-step {
                display: none;
            }
            
            .auth-step.active {
                display: block;
            }
            
            .input-group {
                margin-bottom: 1rem;
            }
            
            .input-group label {
                display: block;
                margin-bottom: 0.5rem;
                color: #333;
                font-weight: 500;
            }
            
            .input-group input {
                width: 100%;
                padding: 0.75rem;
                border: 2px solid #e1e5e9;
                border-radius: 6px;
                font-size: 1rem;
                transition: border-color 0.2s;
            }
            
            .input-group input:focus {
                outline: none;
                border-color: #667eea;
            }
            
            .btn-primary, .btn-secondary, .btn-oauth {
                width: 100%;
                padding: 0.75rem;
                border: none;
                border-radius: 6px;
                font-size: 1rem;
                font-weight: 500;
                cursor: pointer;
                transition: all 0.2s;
                margin-bottom: 0.5rem;
            }
            
            .btn-primary {
                background: #667eea;
                color: white;
            }
            
            .btn-primary:hover {
                background: #5a6fd8;
            }
            
            .btn-secondary {
                background: #f8f9fa;
                color: #333;
                border: 2px solid #e1e5e9;
            }
            
            .btn-oauth {
                display: flex;
                align-items: center;
                justify-content: center;
                gap: 0.5rem;
                background: white;
                color: #333;
                border: 2px solid #e1e5e9;
            }
            
            .btn-oauth.google:hover {
                border-color: #db4437;
            }
            
            .btn-oauth.github:hover {
                border-color: #333;
            }
            
            .divider {
                text-align: center;
                margin: 1.5rem 0;
                color: #666;
                position: relative;
            }
            
            .divider::before {
                content: '';
                position: absolute;
                top: 50%;
                left: 0;
                right: 0;
                height: 1px;
                background: #e1e5e9;
                z-index: 1;
            }
            
            .divider {
                background: white;
                padding: 0 1rem;
                z-index: 2;
                position: relative;
            }
            
            .hidden {
                display: none !important;
            }
            
            .user-info {
                background: #f8f9fa;
                padding: 1rem;
                border-radius: 6px;
                margin-bottom: 1rem;
            }
            
            .passkey-help {
                margin-top: 1.5rem;
                font-size: 0.9rem;
                color: #666;
            }
            
            .passkey-help h4 {
                margin: 0 0 0.5rem 0;
                color: #333;
            }
            
            .passkey-help ul {
                margin: 0;
                padding-left: 1.2rem;
            }
            
            .loading-spinner {
                text-align: center;
                padding: 2rem 0;
            }
            
            .spinner {
                width: 40px;
                height: 40px;
                border: 4px solid #f3f3f3;
                border-top: 4px solid #667eea;
                border-radius: 50%;
                animation: spin 1s linear infinite;
                margin: 0 auto 1rem auto;
            }
            
            @keyframes spin {
                0% { transform: rotate(0deg); }
                100% { transform: rotate(360deg); }
            }
            
            .success-message {
                text-align: center;
            }
            
            .wallet-info {
                background: #f8f9fa;
                padding: 1rem;
                border-radius: 6px;
                margin: 1rem 0;
                text-align: left;
            }
            
            .recovery-info {
                margin-top: 1.5rem;
                text-align: left;
            }
            
            .recovery-data {
                background: #f8f9fa;
                padding: 1rem;
                border-radius: 6px;
                margin: 0.5rem 0;
                font-family: monospace;
                font-size: 0.8rem;
                max-height: 120px;
                overflow-y: auto;
            }
            
            .status-message {
                padding: 0.75rem;
                border-radius: 6px;
                margin-top: 1rem;
                background: #d1ecf1;
                color: #0c5460;
                border: 1px solid #bee5eb;
            }
            
            .error-message {
                padding: 0.75rem;
                border-radius: 6px;
                margin-top: 1rem;
                background: #f8d7da;
                color: #721c24;
                border: 1px solid #f5c6cb;
            }
            
            .icon {
                font-size: 1.2rem;
            }
        `;
        document.head.appendChild(style);
    }

    bindEvents() {
        // Email表单提交
        document.getElementById('email-form').addEventListener('submit', (e) => {
            e.preventDefault();
            this.sendEmailVerification();
        });

        // 验证码验证
        document.getElementById('verify-code-btn').addEventListener('click', () => {
            this.verifyEmailCode();
        });

        // OAuth登录
        document.getElementById('google-login').addEventListener('click', () => {
            this.initiateOAuthLogin('google');
        });

        document.getElementById('github-login').addEventListener('click', () => {
            this.initiateOAuthLogin('github');
        });

        // Passkey创建
        document.getElementById('create-passkey-btn').addEventListener('click', () => {
            this.createPasskey();
        });

        // 下载恢复信息
        document.getElementById('download-recovery').addEventListener('click', () => {
            this.downloadRecoveryInfo();
        });

        // 继续到钱包
        document.getElementById('continue-to-wallet').addEventListener('click', () => {
            this.continueToWallet();
        });

        // 检查OAuth回调
        this.checkOAuthCallback();
    }

    async sendEmailVerification() {
        const email = document.getElementById('email').value;
        this.showStatus('正在发送验证码...');

        try {
            const response = await fetch(`${this.apiBaseUrl}/api/auth/email/send`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ email })
            });

            const result = await response.json();
            
            if (result.success) {
                this.userInfo.email = email;
                this.userInfo.source = 'email';
                document.getElementById('verification-form').classList.remove('hidden');
                this.showStatus('验证码已发送到您的邮箱');
            } else {
                this.showError(result.message || '发送失败');
            }
        } catch (error) {
            this.showError('网络错误，请重试');
        }
    }

    async verifyEmailCode() {
        const code = document.getElementById('verification-code').value;
        const email = this.userInfo.email;

        if (!code || code.length !== 6) {
            this.showError('请输入6位验证码');
            return;
        }

        this.showStatus('正在验证...');

        try {
            const response = await fetch(`${this.apiBaseUrl}/api/auth/email/verify`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ email, code })
            });

            const result = await response.json();
            
            if (result.success) {
                this.userInfo.verified = true;
                this.showPasskeyStep();
            } else {
                this.showError('验证码错误或已过期');
            }
        } catch (error) {
            this.showError('验证失败，请重试');
        }
    }

    async initiateOAuthLogin(provider) {
        this.showStatus(`正在跳转到${provider}登录...`);

        try {
            const response = await fetch(`${this.apiBaseUrl}/api/auth/oauth/${provider}/url`);
            const result = await response.json();
            
            if (result.success) {
                // 保存CSRF token
                localStorage.setItem('oauth_csrf_token', result.csrf_token);
                localStorage.setItem('oauth_provider', provider);
                
                // 跳转到OAuth提供商
                window.location.href = result.auth_url;
            } else {
                this.showError(`${provider}登录配置错误`);
            }
        } catch (error) {
            this.showError('OAuth登录失败');
        }
    }

    async checkOAuthCallback() {
        const urlParams = new URLSearchParams(window.location.search);
        const code = urlParams.get('code');
        const state = urlParams.get('state');
        const provider = localStorage.getItem('oauth_provider');
        const storedCsrfToken = localStorage.getItem('oauth_csrf_token');

        if (code && state && provider) {
            // 验证CSRF token
            if (state !== storedCsrfToken) {
                this.showError('OAuth安全验证失败');
                return;
            }

            this.showStatus('正在处理OAuth登录...');

            try {
                const response = await fetch(`${this.apiBaseUrl}/api/auth/oauth/${provider}/callback`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ code, state })
                });

                const result = await response.json();
                
                if (result.success) {
                    this.userInfo = {
                        email: result.user_info.email,
                        name: result.user_info.name,
                        source: result.user_info.provider,
                        userId: result.user_info.user_id,
                        verified: true
                    };

                    // 清理localStorage
                    localStorage.removeItem('oauth_csrf_token');
                    localStorage.removeItem('oauth_provider');

                    // 清理URL参数
                    window.history.replaceState({}, document.title, window.location.pathname);

                    this.showPasskeyStep();
                } else {
                    this.showError('OAuth登录失败');
                }
            } catch (error) {
                this.showError('OAuth处理失败');
            }
        }
    }

    showPasskeyStep() {
        document.getElementById('email-step').classList.remove('active');
        document.getElementById('passkey-step').classList.add('active');
        
        document.getElementById('user-email').textContent = this.userInfo.email;
        document.getElementById('user-source').textContent = 
            this.userInfo.source === 'email' ? '邮箱验证' : `${this.userInfo.source.toUpperCase()}登录`;
        
        this.currentStep = 'passkey';
        this.clearMessages();
    }

    async createPasskey() {
        this.showStatus('正在生成Passkey挑战...');

        try {
            // 生成注册选项
            const response = await fetch(`${this.apiBaseUrl}/api/webauthn/register/begin`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    user_id: this.userInfo.userId || this.generateUserId(),
                    user_name: this.userInfo.email,
                    user_display_name: this.userInfo.name || this.userInfo.email,
                    rp_name: 'AirAccount',
                    rp_id: window.location.hostname
                })
            });

            const registerOptions = await response.json();
            
            if (!registerOptions.success) {
                throw new Error(registerOptions.error || 'Failed to generate registration options');
            }

            this.sessionId = registerOptions.session_id;
            this.showStatus('请使用您的设备完成Passkey创建...');

            // 调用WebAuthn API
            const credential = await navigator.credentials.create({
                publicKey: {
                    ...registerOptions.options,
                    challenge: this.base64ToArrayBuffer(registerOptions.options.challenge),
                    user: {
                        ...registerOptions.options.user,
                        id: this.base64ToArrayBuffer(registerOptions.options.user.id)
                    }
                }
            });

            // 验证注册响应
            const verificationResponse = await fetch(`${this.apiBaseUrl}/api/webauthn/register/finish`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    session_id: this.sessionId,
                    registration_response: {
                        id: credential.id,
                        rawId: this.arrayBufferToBase64(credential.rawId),
                        response: {
                            clientDataJSON: this.arrayBufferToBase64(credential.response.clientDataJSON),
                            attestationObject: this.arrayBufferToBase64(credential.response.attestationObject)
                        },
                        type: credential.type
                    }
                })
            });

            const verificationResult = await verificationResponse.json();
            
            if (verificationResult.success && verificationResult.verified) {
                this.userInfo.passkeyCredentialId = credential.id;
                this.showWalletCreation();
            } else {
                throw new Error('Passkey注册验证失败');
            }

        } catch (error) {
            if (error.name === 'NotAllowedError') {
                this.showError('Passkey创建被取消或设备不支持');
            } else {
                this.showError(`Passkey创建失败: ${error.message}`);
            }
        }
    }

    async showWalletCreation() {
        document.getElementById('passkey-step').classList.remove('active');
        document.getElementById('wallet-step').classList.add('active');
        this.currentStep = 'wallet';
        
        try {
            // 创建钱包
            const response = await fetch(`${this.apiBaseUrl}/api/account/create`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    email: this.userInfo.email,
                    passkey_credential_id: this.userInfo.passkeyCredentialId,
                    passkey_public_key_base64: 'dummy_placeholder' // 实际公钥由WebAuthn管理
                })
            });

            const result = await response.json();
            
            if (result.success !== false) {
                this.userInfo.walletId = result.wallet_id;
                this.userInfo.ethereumAddress = result.ethereum_address;
                this.userInfo.teeDeviceId = result.tee_device_id;
                
                this.showWalletSuccess();
            } else {
                throw new Error(result.error || '钱包创建失败');
            }
        } catch (error) {
            this.showError(`钱包创建失败: ${error.message}`);
        }
    }

    showWalletSuccess() {
        document.querySelector('.loading-spinner').classList.add('hidden');
        document.getElementById('wallet-result').classList.remove('hidden');
        
        document.getElementById('wallet-id').textContent = this.userInfo.walletId;
        document.getElementById('ethereum-address').textContent = this.userInfo.ethereumAddress;
        document.getElementById('tee-device-id').textContent = this.userInfo.teeDeviceId;
        
        const recoveryInfo = {
            email: this.userInfo.email,
            passkeyCredentialId: this.userInfo.passkeyCredentialId,
            walletId: this.userInfo.walletId,
            ethereumAddress: this.userInfo.ethereumAddress,
            teeDeviceId: this.userInfo.teeDeviceId,
            createdAt: new Date().toISOString()
        };
        
        document.getElementById('recovery-json').textContent = JSON.stringify(recoveryInfo, null, 2);
        this.recoveryInfo = recoveryInfo;
    }

    downloadRecoveryInfo() {
        const dataStr = JSON.stringify(this.recoveryInfo, null, 2);
        const dataBlob = new Blob([dataStr], { type: 'application/json' });
        const url = URL.createObjectURL(dataBlob);
        
        const link = document.createElement('a');
        link.href = url;
        link.download = `airaccount-recovery-${this.userInfo.walletId}.json`;
        link.click();
        
        URL.revokeObjectURL(url);
        this.showStatus('恢复信息已下载');
    }

    continueToWallet() {
        // 跳转到钱包界面或触发回调
        if (this.onComplete) {
            this.onComplete(this.userInfo);
        } else {
            alert('注册完成！钱包已创建。');
        }
    }

    // 工具方法
    generateUserId() {
        return 'user_' + Math.random().toString(36).substr(2, 9);
    }

    base64ToArrayBuffer(base64) {
        const binaryString = atob(base64);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        return bytes.buffer;
    }

    arrayBufferToBase64(buffer) {
        const bytes = new Uint8Array(buffer);
        let binary = '';
        for (let i = 0; i < bytes.byteLength; i++) {
            binary += String.fromCharCode(bytes[i]);
        }
        return btoa(binary);
    }

    showStatus(message) {
        const statusEl = document.getElementById('status-message');
        const errorEl = document.getElementById('error-message');
        
        statusEl.textContent = message;
        statusEl.classList.remove('hidden');
        errorEl.classList.add('hidden');
    }

    showError(message) {
        const statusEl = document.getElementById('status-message');
        const errorEl = document.getElementById('error-message');
        
        errorEl.textContent = message;
        errorEl.classList.remove('hidden');
        statusEl.classList.add('hidden');
    }

    clearMessages() {
        document.getElementById('status-message').classList.add('hidden');
        document.getElementById('error-message').classList.add('hidden');
    }

    // 设置完成回调
    onComplete(callback) {
        this.onComplete = callback;
    }
}

// 使用示例
// const auth = new AirAccountAuth({
//     apiBaseUrl: 'http://localhost:3001'
// });
// 
// auth.onComplete((userInfo) => {
//     console.log('用户注册完成:', userInfo);
//     // 跳转到钱包界面
// });

// 如果在浏览器环境中，自动初始化
if (typeof window !== 'undefined') {
    window.AirAccountAuth = AirAccountAuth;
    
    // 可以在DOM加载完成后自动初始化
    document.addEventListener('DOMContentLoaded', () => {
        if (document.getElementById('airaccount-auto-init')) {
            new AirAccountAuth();
        }
    });
}