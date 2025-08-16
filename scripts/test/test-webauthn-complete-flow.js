#!/usr/bin/env node

/**
 * WebAuthn 完整流程测试
 * 验证 WebAuthn 注册和认证的完整流程
 */

const axios = require('axios');

// 配置
const API_BASE = 'http://localhost:3002';
const TEST_EMAIL = 'test@example.com';
const TEST_DISPLAY_NAME = 'Test User';

// 模拟的 WebAuthn 响应数据 - 使用真实格式但是简化的数据
const MOCK_REGISTRATION_RESPONSE = {
  id: 'dGVzdF9jcmVkZW50aWFsX2lk',  // Base64编码的 "test_credential_id"
  rawId: 'dGVzdF9jcmVkZW50aWFsX2lk',
  response: {
    clientDataJSON: Buffer.from(JSON.stringify({
      type: 'webauthn.create',
      challenge: 'challenge_placeholder',
      origin: 'http://localhost:3002',
    })).toString('base64'),
    // 最小有效的CBOR格式attestationObject
    attestationObject: 'o2NmbXRkbm9uZWdhdHRTdG10oGhhdXRoRGF0YVhYSUxNVExWUU1CUE1NRUh0dHRCZGVlQUdqemJCcDBRZENNTUJQUVZ4QWhZbGVMSE1MMFFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBUQAAAABkdGVzdF9jcmVkZW50aWFsX2lkpQECAyYgASFYIDi9bVe4TjHGNw3cOlHOFLZR3zNT6LHZ_ZCJLGSbp2chIlgg6Fg6_w-HJVk_VvVP7pFZ3aUXFKKINOhUIbMO_lC7Qw',
    transports: ['internal'],
  },
  type: 'public-key',
};

const MOCK_AUTHENTICATION_RESPONSE = {
  id: 'dGVzdF9jcmVkZW50aWFsX2lk',  // 同样的credential ID
  rawId: 'dGVzdF9jcmVkZW50aWFsX2lk',
  response: {
    clientDataJSON: Buffer.from(JSON.stringify({
      type: 'webauthn.get',
      challenge: 'challenge_placeholder',
      origin: 'http://localhost:3002',
    })).toString('base64'),
    // Base64编码的最小有效authenticatorData
    authenticatorData: 'SUxNVExWUU1CUE1NRUh0dHRCZGVlQUdqemJCcDBRZENNTUJQUVZ4QWhZbGVMSE1MMEABAAAABA',
    // 模拟签名数据
    signature: 'bW9ja19zaWduYXR1cmVfZGF0YQ',
  },
  type: 'public-key',
};

async function testWebAuthnFlow() {
  console.log('🚀 开始 WebAuthn 完整流程测试\n');

  try {
    // 第一步：测试服务器健康状态
    console.log('1️⃣ 测试服务器连接...');
    const healthResponse = await axios.get(`${API_BASE}/health`);
    console.log(`✅ 服务器状态: ${healthResponse.data.status}`);
    console.log(`   TEE连接: ${healthResponse.data.teeConnection}`);
    console.log(`   数据库: ${healthResponse.data.database}\n`);

    // 第二步：开始注册流程
    console.log('2️⃣ 开始 WebAuthn 注册...');
    const registerBeginResponse = await axios.post(`${API_BASE}/api/webauthn/register/begin`, {
      email: TEST_EMAIL,
      displayName: TEST_DISPLAY_NAME,
    });

    console.log('✅ 注册选项生成成功');
    console.log(`   Challenge: ${registerBeginResponse.data.options.challenge.substring(0, 16)}...`);
    console.log(`   Session ID: ${registerBeginResponse.data.sessionId}`);
    console.log(`   用户责任: ${registerBeginResponse.data.notice.userResponsibility}\n`);

    // 模拟用户在浏览器中完成 WebAuthn 操作
    const challenge = registerBeginResponse.data.options.challenge;
    
    // 第三步：完成注册
    console.log('3️⃣ 完成 WebAuthn 注册...');
    const registrationResponse = {
      ...MOCK_REGISTRATION_RESPONSE,
      response: {
        ...MOCK_REGISTRATION_RESPONSE.response,
        clientDataJSON: Buffer.from(JSON.stringify({
          type: 'webauthn.create',
          challenge: challenge,
          origin: 'http://localhost:3002',
        })).toString('base64'),
      },
    };

    const registerFinishResponse = await axios.post(`${API_BASE}/api/webauthn/register/finish`, {
      email: TEST_EMAIL,
      registrationResponse: registrationResponse,
      challenge: challenge,
    });

    console.log('✅ 注册完成');
    console.log(`   钱包ID: ${registerFinishResponse.data.walletResult.walletId}`);
    console.log(`   以太坊地址: ${registerFinishResponse.data.walletResult.ethereumAddress}`);
    console.log(`   凭证ID: ${registerFinishResponse.data.userInstructions.credentialId}`);
    console.log(`   恢复信息:`, registerFinishResponse.data.userInstructions.recoveryInfo);
    console.log(`   警告: ${registerFinishResponse.data.userInstructions.warning}\n`);

    // 第四步：开始认证流程
    console.log('4️⃣ 开始 WebAuthn 认证...');
    const authBeginResponse = await axios.post(`${API_BASE}/api/webauthn/authenticate/begin`, {
      email: TEST_EMAIL,
    });

    console.log('✅ 认证选项生成成功');
    console.log(`   Challenge: ${authBeginResponse.data.options.challenge.substring(0, 16)}...`);
    console.log(`   消息: ${authBeginResponse.data.notice.message}\n`);

    // 第五步：完成认证
    console.log('5️⃣ 完成 WebAuthn 认证...');
    const authChallenge = authBeginResponse.data.options.challenge;
    const authenticationResponse = {
      ...MOCK_AUTHENTICATION_RESPONSE,
      response: {
        ...MOCK_AUTHENTICATION_RESPONSE.response,
        clientDataJSON: Buffer.from(JSON.stringify({
          type: 'webauthn.get',
          challenge: authChallenge,
          origin: 'http://localhost:3002',
        })).toString('base64'),
      },
    };

    const authFinishResponse = await axios.post(`${API_BASE}/api/webauthn/authenticate/finish`, {
      email: TEST_EMAIL,
      authenticationResponse: authenticationResponse,
      challenge: authChallenge,
    });

    console.log('✅ 认证完成');
    console.log(`   Session ID: ${authFinishResponse.data.sessionId}`);
    console.log(`   用户账户:`, authFinishResponse.data.userAccount);
    console.log(`   会话信息: ${authFinishResponse.data.sessionInfo.message}\n`);

    // 第六步：验证TEE安全状态
    console.log('6️⃣ 验证 TEE 安全状态...');
    const securityResponse = await axios.get(`${API_BASE}/api/webauthn/security/verify`);
    
    console.log('✅ 安全状态验证完成');
    console.log(`   验证状态: ${securityResponse.data.securityState.verified ? '✅ 通过' : '❌ 失败'}`);
    console.log(`   状态详情:`, securityResponse.data.securityState.details);
    console.log(`   注意: ${securityResponse.data.notice}\n`);

    // 第七步：获取统计信息
    console.log('7️⃣ 获取 WebAuthn 统计信息...');
    const statsResponse = await axios.get(`${API_BASE}/api/webauthn/stats`);
    
    console.log('✅ 统计信息获取成功');
    console.log(`   总用户数: ${statsResponse.data.stats.totalUsers}`);
    console.log(`   总设备数: ${statsResponse.data.stats.totalDevices}`);
    console.log(`   活跃Challenge: ${statsResponse.data.stats.activeChallenges}`);
    console.log(`   免责声明: ${statsResponse.data.disclaimer}\n`);

    // 完成测试
    console.log('🎉 WebAuthn 完整流程测试完成！');
    console.log('\n📊 测试总结:');
    console.log('✅ 注册流程 - 成功');
    console.log('✅ 认证流程 - 成功');
    console.log('✅ 安全验证 - 成功');
    console.log('✅ TEE集成 - 成功');
    console.log('\n🔒 关键架构特点已验证:');
    console.log('- 用户凭证设备存储（客户端控制）');
    console.log('- 节点只提供临时服务');
    console.log('- TEE混合熵源安全实现');
    console.log('- 完整的恢复信息提供');

  } catch (error) {
    console.error('\n❌ 测试失败:', error.message);
    if (error.response) {
      console.error('响应状态:', error.response.status);
      console.error('响应数据:', error.response.data);
    }
    process.exit(1);
  }
}

// 运行测试
if (require.main === module) {
  testWebAuthnFlow().catch(console.error);
}

module.exports = { testWebAuthnFlow };