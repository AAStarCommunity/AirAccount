const { ethers } = require('ethers');

// 模拟与测试程序相同的参数
const userOpHash = '0x8dfca86d8053ca45deb4661f4dd97500536aa0ce31f2c03aa73e573b515173af';
const accountId = 'test-account-phase1-real';
const userSignature = '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1b';
const nonce = 805509; // 示例值，应从测试输出中获取
const timestamp = 1756805507; // 示例值，应从测试输出中获取

console.log('🔍 调试 Paymaster 签名验证...');
console.log('参数:');
console.log('  UserOp Hash:', userOpHash);
console.log('  Account ID:', accountId);
console.log('  User Signature:', userSignature);
console.log('  Nonce:', nonce);
console.log('  Timestamp:', timestamp);

// 计算用户签名哈希
const userSigHash = ethers.keccak256(ethers.toUtf8Bytes(userSignature));
console.log('  User Sig Hash:', userSigHash);

// 使用 ethers.js 的 solidityPackedKeccak256
const messageHash = ethers.solidityPackedKeccak256(
  ['bytes32', 'string', 'bytes32', 'uint256', 'uint256'],
  [
    userOpHash,
    accountId,
    userSigHash,
    nonce,
    timestamp
  ]
);

console.log('\n📝 计算结果:');
console.log('  Message Hash:', messageHash);

// 创建测试钱包
const testWallet = ethers.Wallet.createRandom();
console.log('  Test Wallet Address:', testWallet.address);

// 签名消息
const signature = testWallet.signMessageSync(ethers.getBytes(messageHash));
console.log('  Test Signature:', signature);

// 验证签名
const recoveredAddress = ethers.verifyMessage(ethers.getBytes(messageHash), signature);
console.log('  Recovered Address:', recoveredAddress);
console.log('  Matches Wallet:', recoveredAddress.toLowerCase() === testWallet.address.toLowerCase());