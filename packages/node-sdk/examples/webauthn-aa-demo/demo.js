/**
 * AirAccount WebAuthn + Account Abstraction Demo
 * 基于passkey-demo和abstract-account最佳实践
 */

// 配置
const CONFIG = {
    apiBaseUrl: 'http://localhost:3002',
    rpName: 'AirAccount Demo',
    rpId: 'localhost',
    origin: 'http://localhost:3002',
    chainId: 1,
    entryPointAddress: '0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789', // ERC-4337 EntryPoint
    factoryAddress: '0x9406Cc6185a346906296840746125a0E44976454' // Mock factory
};

// 全局状态
let currentUser = null;
let sessionToken = null;
let accountInfo = null;

// 初始化
document.addEventListener('DOMContentLoaded', async () => {
    await checkBrowserSupport();
    setupEventListeners();
    log('Demo initialized', 'info');
});

// === 浏览器支持检查 ===

async function checkBrowserSupport() {
    const supportStatus = document.getElementById('supportStatus');
    const checks = [
        { id: 'webauthnSupport', test: () => !!window.PublicKeyCredential, name: 'WebAuthn API' },
        { id: 'platformAuthSupport', test: checkPlatformAuth, name: 'Platform Authenticator' },
        { id: 'credentialCreateSupport', test: () => !!navigator.credentials?.create, name: 'Credential Creation' },
        { id: 'userVerificationSupport', test: checkUserVerification, name: 'User Verification' }
    ];

    let allSupported = true;

    for (const check of checks) {
        const element = document.getElementById(check.id);
        try {
            const supported = await check.test();
            if (supported) {
                element.style.color = '#22543d';
                element.querySelector('::before')?.style.setProperty('content', '"✅"');
            } else {
                element.style.color = '#742a2a';
                element.innerHTML = `❌ ${check.name} (Not Supported)`;
                allSupported = false;
            }
        } catch (error) {
            element.style.color = '#742a2a';
            element.innerHTML = `❌ ${check.name} (Error: ${error.message})`;
            allSupported = false;
        }
    }

    if (allSupported) {
        supportStatus.className = 'status success';
        supportStatus.textContent = '✅ All WebAuthn features are supported on this device!';
    } else {
        supportStatus.className = 'status error';
        supportStatus.textContent = '❌ Some WebAuthn features are not supported. The demo may not work properly.';
    }
}

async function checkPlatformAuth() {
    if (!window.PublicKeyCredential?.isUserVerifyingPlatformAuthenticatorAvailable) {
        return false;
    }
    return await PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable();
}

async function checkUserVerification() {
    return !!navigator.credentials && typeof navigator.credentials.create === 'function';
}

// === 用户注册 ===

async function registerUser() {
    const email = document.getElementById('regEmail').value;
    const displayName = document.getElementById('regDisplayName').value;
    
    if (!email || !displayName) {
        showStatus('registrationStatus', 'Please fill in all fields', 'error');
        return;
    }

    const registerBtn = document.getElementById('registerBtn');
    const originalText = registerBtn.querySelector('.btn-text').textContent;
    
    try {
        // 显示加载状态
        registerBtn.disabled = true;
        registerBtn.querySelector('.btn-text').innerHTML = '<span class="loading"></span> Creating Account...';
        showStatus('registrationStatus', 'Starting passkey registration...', 'info');
        
        log(`Starting registration for ${email}`, 'info');

        // 步骤1: 获取注册选项
        const regOptionsResponse = await fetch(`${CONFIG.apiBaseUrl}/webauthn/register/begin`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                email: email,
                displayName: displayName
            })
        });

        if (!regOptionsResponse.ok) {
            throw new Error(`Registration options request failed: ${regOptionsResponse.statusText}`);
        }

        const regOptionsData = await regOptionsResponse.json();
        
        if (!regOptionsData.success) {
            throw new Error(regOptionsData.error || 'Failed to get registration options');
        }

        log('Registration options received', 'success');
        showStatus('registrationStatus', 'Creating passkey...', 'info');

        // 步骤2: 创建WebAuthn凭证
        const registrationResponse = await SimpleWebAuthnBrowser.startRegistration(regOptionsData.options);
        
        log('Passkey created successfully', 'success');
        showStatus('registrationStatus', 'Verifying registration...', 'info');

        // 步骤3: 验证注册
        const verificationResponse = await fetch(`${CONFIG.apiBaseUrl}/webauthn/register/finish`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                email: email,
                registrationResponse: registrationResponse,
                challenge: regOptionsData.options.challenge
            })
        });

        if (!verificationResponse.ok) {
            throw new Error(`Registration verification failed: ${verificationResponse.statusText}`);
        }

        const verificationData = await verificationResponse.json();
        
        if (!verificationData.success) {
            throw new Error(verificationData.error || 'Registration verification failed');
        }

        // 保存用户状态
        currentUser = { email, displayName };
        sessionToken = verificationData.sessionId || regOptionsData.sessionId;
        
        log('Registration completed successfully', 'success');
        showStatus('registrationStatus', '✅ Registration successful!', 'success');
        
        // 显示注册结果
        displayRegistrationResult(verificationData);
        
    } catch (error) {
        log(`Registration failed: ${error.message}`, 'error');
        
        // 处理特定错误
        let errorMessage = 'Registration failed';
        if (error.name === 'NotAllowedError') {
            errorMessage = 'User cancelled the registration process';
        } else if (error.name === 'NotSupportedError') {
            errorMessage = 'This device does not support passkeys';
        } else {
            errorMessage = error.message;
        }
        
        showStatus('registrationStatus', `❌ ${errorMessage}`, 'error');
        
    } finally {
        // 恢复按钮状态
        registerBtn.disabled = false;
        registerBtn.querySelector('.btn-text').textContent = originalText;
    }
}

function displayRegistrationResult(data) {
    const resultDiv = document.getElementById('registrationResult');
    
    document.getElementById('regUserId').textContent = data.walletResult?.teeDeviceId || 'N/A';
    document.getElementById('regCredentialId').textContent = data.userInstructions?.credentialId || 'N/A';
    document.getElementById('regWalletAddress').textContent = data.walletResult?.ethereumAddress || 'N/A';
    document.getElementById('regSessionToken').textContent = sessionToken || 'N/A';
    
    resultDiv.classList.remove('hidden');
}

// === 用户认证 ===

async function authenticateUser() {
    const email = document.getElementById('authEmail').value;
    
    if (!email) {
        showStatus('authStatus', 'Please enter your email address', 'error');
        return;
    }

    await performAuthentication(email);
}

async function passwordlessAuth() {
    log('Starting passwordless authentication', 'info');
    await performAuthentication(); // 不传email实现无密码登录
}

async function performAuthentication(email = null) {
    const authBtn = document.getElementById('authBtn');
    const passwordlessBtn = document.getElementById('passwordlessBtn');
    const originalAuthText = authBtn.querySelector('.btn-text').textContent;
    const originalPasswordlessText = passwordlessBtn.querySelector('.btn-text').textContent;
    
    try {
        // 显示加载状态
        authBtn.disabled = true;
        passwordlessBtn.disabled = true;
        authBtn.querySelector('.btn-text').innerHTML = '<span class="loading"></span> Authenticating...';
        passwordlessBtn.querySelector('.btn-text').innerHTML = '<span class="loading"></span> Authenticating...';
        
        const authType = email ? `email: ${email}` : 'passwordless mode';
        showStatus('authStatus', `Starting authentication (${authType})...`, 'info');
        log(`Starting authentication (${authType})`, 'info');

        // 步骤1: 获取认证选项
        const authOptionsResponse = await fetch(`${CONFIG.apiBaseUrl}/webauthn/authenticate/begin`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email })
        });

        if (!authOptionsResponse.ok) {
            throw new Error(`Authentication options request failed: ${authOptionsResponse.statusText}`);
        }

        const authOptionsData = await authOptionsResponse.json();
        
        if (!authOptionsData.success) {
            throw new Error(authOptionsData.error || 'Failed to get authentication options');
        }

        log('Authentication options received', 'success');
        showStatus('authStatus', 'Waiting for biometric verification...', 'info');

        // 步骤2: 执行WebAuthn认证
        const authResponse = await SimpleWebAuthnBrowser.startAuthentication(authOptionsData.options);
        
        log('Biometric verification successful', 'success');
        showStatus('authStatus', 'Verifying authentication...', 'info');

        // 步骤3: 验证认证响应
        const verificationResponse = await fetch(`${CONFIG.apiBaseUrl}/webauthn/authenticate/finish`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                email: email || authResponse.response.userHandle,
                authenticationResponse: authResponse,
                challenge: authOptionsData.options.challenge
            })
        });

        if (!verificationResponse.ok) {
            throw new Error(`Authentication verification failed: ${verificationResponse.statusText}`);
        }

        const verificationData = await verificationResponse.json();
        
        if (!verificationData.success) {
            throw new Error(verificationData.error || 'Authentication verification failed');
        }

        // 保存认证状态
        currentUser = verificationData.userAccount;
        sessionToken = verificationData.sessionId;
        
        log('Authentication completed successfully', 'success');
        showStatus('authStatus', '✅ Authentication successful!', 'success');
        
        // 显示认证结果
        displayAuthenticationResult(verificationData);
        
    } catch (error) {
        log(`Authentication failed: ${error.message}`, 'error');
        
        // 处理特定错误
        let errorMessage = 'Authentication failed';
        if (error.name === 'NotAllowedError') {
            errorMessage = 'User cancelled the authentication process';
        } else if (error.name === 'InvalidStateError') {
            errorMessage = 'No passkey found for this device';
        } else {
            errorMessage = error.message;
        }
        
        showStatus('authStatus', `❌ ${errorMessage}`, 'error');
        
    } finally {
        // 恢复按钮状态
        authBtn.disabled = false;
        passwordlessBtn.disabled = false;
        authBtn.querySelector('.btn-text').textContent = originalAuthText;
        passwordlessBtn.querySelector('.btn-text').textContent = originalPasswordlessText;
    }
}

function displayAuthenticationResult(data) {
    const resultDiv = document.getElementById('authResult');
    
    document.getElementById('authUserEmail').textContent = data.userAccount?.email || currentUser?.email || 'N/A';
    document.getElementById('authSessionToken').textContent = sessionToken || 'N/A';
    document.getElementById('authWalletId').textContent = data.userAccount?.walletId || 'N/A';
    document.getElementById('authEthAddress').textContent = data.userAccount?.ethereumAddress || 'N/A';
    
    resultDiv.classList.remove('hidden');
}

// === 账户抽象功能 ===

async function checkAccountBalance() {
    if (!sessionToken) {
        showStatus('aaStatus', 'Please authenticate first', 'error');
        return;
    }

    try {
        showStatus('aaStatus', 'Checking account balance...', 'info');
        log('Checking account balance', 'info');
        
        // 模拟账户余额查询
        const mockBalance = {
            native: '0.1',
            tokens: [
                { symbol: 'USDC', balance: '100.0', address: '0xA0b86...' },
                { symbol: 'LINK', balance: '5.0', address: '0x514910...' }
            ]
        };
        
        log(`Account balance: ${mockBalance.native} ETH`, 'success');
        showStatus('aaStatus', `✅ Balance: ${mockBalance.native} ETH`, 'success');
        
        // 更新显示
        document.getElementById('aaBalance').textContent = `${mockBalance.native} ETH`;
        
    } catch (error) {
        log(`Balance check failed: ${error.message}`, 'error');
        showStatus('aaStatus', `❌ Balance check failed: ${error.message}`, 'error');
    }
}

async function getAccountInfo() {
    if (!sessionToken) {
        showStatus('aaStatus', 'Please authenticate first', 'error');
        return;
    }

    try {
        showStatus('aaStatus', 'Getting account information...', 'info');
        log('Getting account information', 'info');
        
        // 模拟账户信息
        accountInfo = {
            address: '0x742d35Cc6634C0532925a3b8D4521FB8d',
            nonce: 0,
            isDeployed: false,
            owner: currentUser?.email || 'unknown',
            recoveryMethod: 'passkey'
        };
        
        log('Account information retrieved', 'success');
        showStatus('aaStatus', '✅ Account information retrieved', 'success');
        
        // 显示账户信息
        displayAccountInfo(accountInfo);
        
    } catch (error) {
        log(`Get account info failed: ${error.message}`, 'error');
        showStatus('aaStatus', `❌ Get account info failed: ${error.message}`, 'error');
    }
}

function displayAccountInfo(info) {
    const accountDiv = document.getElementById('accountInfo');
    
    document.getElementById('aaAddress').textContent = info.address;
    document.getElementById('aaNonce').textContent = info.nonce;
    document.getElementById('aaDeployed').textContent = info.isDeployed ? 'Yes' : 'No';
    
    accountDiv.classList.remove('hidden');
}

async function deployAccount() {
    if (!accountInfo) {
        showStatus('aaStatus', 'Please get account info first', 'error');
        return;
    }

    try {
        showStatus('aaStatus', 'Deploying account...', 'info');
        log('Deploying account contract', 'info');
        
        // 模拟账户部署
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        accountInfo.isDeployed = true;
        accountInfo.nonce = 1;
        
        log('Account deployed successfully', 'success');
        showStatus('aaStatus', '✅ Account deployed successfully', 'success');
        
        // 更新显示
        displayAccountInfo(accountInfo);
        
    } catch (error) {
        log(`Account deployment failed: ${error.message}`, 'error');
        showStatus('aaStatus', `❌ Account deployment failed: ${error.message}`, 'error');
    }
}

async function sendTransaction() {
    await performTransaction(false);
}

async function sendWithPaymaster() {
    await performTransaction(true);
}

async function performTransaction(usePaymaster = false) {
    if (!sessionToken) {
        showStatus('aaStatus', 'Please authenticate first', 'error');
        return;
    }

    const to = document.getElementById('txTo').value;
    const value = document.getElementById('txValue').value;
    const data = document.getElementById('txData').value;

    if (!to) {
        showStatus('aaStatus', 'Please enter a recipient address', 'error');
        return;
    }

    try {
        const paymasterText = usePaymaster ? ' (with Paymaster)' : '';
        showStatus('aaStatus', `Sending transaction${paymasterText}...`, 'info');
        log(`Sending transaction${paymasterText}`, 'info');
        
        // 构建交易
        const transaction = {
            to,
            value: value ? ethers.parseEther(value).toString() : '0',
            data: data || '0x'
        };
        
        log(`Transaction details: ${JSON.stringify(transaction)}`, 'info');
        
        // 模拟交易签名和发送
        await new Promise(resolve => setTimeout(resolve, 3000));
        
        const mockTxHash = '0x' + Array.from({length: 64}, () => Math.floor(Math.random() * 16).toString(16)).join('');
        
        log(`Transaction sent: ${mockTxHash}`, 'success');
        showStatus('aaStatus', `✅ Transaction sent: ${mockTxHash.substring(0, 10)}...`, 'success');
        
    } catch (error) {
        log(`Transaction failed: ${error.message}`, 'error');
        showStatus('aaStatus', `❌ Transaction failed: ${error.message}`, 'error');
    }
}

// === TEE安全验证 ===

async function verifyTEESecurity() {
    try {
        showStatus('securityStatus', 'Verifying TEE security state...', 'info');
        log('Starting TEE security verification', 'info');
        
        const response = await fetch(`${CONFIG.apiBaseUrl}/webauthn/security/verify`);
        
        if (!response.ok) {
            throw new Error(`Security verification request failed: ${response.statusText}`);
        }
        
        const data = await response.json();
        
        if (!data.success) {
            throw new Error(data.error || 'Security verification failed');
        }
        
        log('TEE security verification completed', 'success');
        showStatus('securityStatus', '✅ TEE security verification completed', 'success');
        
        // 显示安全信息
        displaySecurityInfo(data.securityState);
        
    } catch (error) {
        log(`TEE security verification failed: ${error.message}`, 'error');
        showStatus('securityStatus', `❌ TEE verification failed: ${error.message}`, 'error');
    }
}

async function testHybridEntropy() {
    try {
        showStatus('securityStatus', 'Testing hybrid entropy generation...', 'info');
        log('Testing hybrid entropy generation', 'info');
        
        // 模拟混合熵源测试
        await new Promise(resolve => setTimeout(resolve, 1500));
        
        const entropyTest = {
            factorySeed: '✅ PASS',
            teeRandom: '✅ PASS',
            hybridCombination: '✅ PASS',
            keyDerivation: '✅ PASS'
        };
        
        log('Hybrid entropy test completed successfully', 'success');
        showStatus('securityStatus', '✅ Hybrid entropy test passed', 'success');
        
        // 显示测试结果
        Object.entries(entropyTest).forEach(([key, value]) => {
            log(`${key}: ${value}`, 'success');
        });
        
    } catch (error) {
        log(`Hybrid entropy test failed: ${error.message}`, 'error');
        showStatus('securityStatus', `❌ Hybrid entropy test failed: ${error.message}`, 'error');
    }
}

function displaySecurityInfo(securityState) {
    const securityDiv = document.getElementById('securityInfo');
    
    document.getElementById('teeVerified').textContent = securityState.verified ? 'Yes' : 'No';
    document.getElementById('teeEntropy').textContent = securityState.details?.tee_entropy || 'N/A';
    document.getElementById('memoryProtection').textContent = securityState.details?.memory_protection || 'N/A';
    document.getElementById('hybridEntropy').textContent = securityState.details?.hybrid_entropy || 'N/A';
    
    securityDiv.classList.remove('hidden');
}

// === 会话管理 ===

async function logout() {
    if (!sessionToken) {
        log('No active session to logout', 'warning');
        return;
    }
    
    try {
        log('Logging out...', 'info');
        
        // 调用服务器登出接口
        await fetch(`${CONFIG.apiBaseUrl}/auth/logout`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ sessionId: sessionToken })
        });
        
        // 清理本地状态
        currentUser = null;
        sessionToken = null;
        accountInfo = null;
        
        // 隐藏结果显示
        document.getElementById('registrationResult').classList.add('hidden');
        document.getElementById('authResult').classList.add('hidden');
        document.getElementById('accountInfo').classList.add('hidden');
        document.getElementById('securityInfo').classList.add('hidden');
        
        // 清空表单
        document.getElementById('regEmail').value = '';
        document.getElementById('regDisplayName').value = '';
        document.getElementById('authEmail').value = '';
        
        log('Logout successful', 'success');
        
    } catch (error) {
        log(`Logout failed: ${error.message}`, 'error');
    }
}

function clearDemo() {
    // 清理所有演示数据
    logout();
    clearLog();
    
    // 清理状态显示
    ['registrationStatus', 'authStatus', 'aaStatus', 'securityStatus'].forEach(id => {
        document.getElementById(id).innerHTML = '';
    });
    
    log('Demo data cleared', 'info');
}

// === 工具函数 ===

function showStatus(elementId, message, type) {
    const element = document.getElementById(elementId);
    element.className = `status ${type}`;
    element.textContent = message;
}

function log(message, type = 'info') {
    const logArea = document.getElementById('logArea');
    const timestamp = new Date().toLocaleTimeString();
    const logEntry = document.createElement('div');
    logEntry.className = `log-entry ${type}`;
    logEntry.textContent = `[${timestamp}] ${message}`;
    
    logArea.appendChild(logEntry);
    logArea.scrollTop = logArea.scrollHeight;
}

function clearLog() {
    document.getElementById('logArea').innerHTML = '';
}

function setupEventListeners() {
    // 回车键监听
    document.getElementById('regEmail').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') registerUser();
    });
    
    document.getElementById('authEmail').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') authenticateUser();
    });
}

// 导出函数供HTML调用
window.registerUser = registerUser;
window.authenticateUser = authenticateUser;
window.passwordlessAuth = passwordlessAuth;
window.checkAccountBalance = checkAccountBalance;
window.getAccountInfo = getAccountInfo;
window.deployAccount = deployAccount;
window.sendTransaction = sendTransaction;
window.sendWithPaymaster = sendWithPaymaster;
window.verifyTEESecurity = verifyTEESecurity;
window.testHybridEntropy = testHybridEntropy;
window.logout = logout;
window.clearDemo = clearDemo;
window.clearLog = clearLog;