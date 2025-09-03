import { ethers } from 'ethers';

// 模拟与测试程序相同的参数
const userOpHash = '0x8dfca86d8053ca45deb4661f4dd97500536aa0ce31f2c03aa73e573b515173af';
const accountId = 'test-account-phase1-real';
const userSignature = '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1b';
const nonce = 805841; // 从最新测试输出获取的实际值 (0x000c4f61)
const timestamp = 1756805857; // 从最新测试输出获取的实际值 (0x68b6be61)

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

console.log('\n🔧 为了调试 Rust 实现，打印详细的字节数据:');
console.log('  UserOp Hash Bytes:', ethers.getBytes(userOpHash));
console.log('  Account ID Bytes:', ethers.toUtf8Bytes(accountId));
console.log('  User Sig Hash Bytes:', ethers.getBytes(userSigHash));
console.log('  Nonce as 32-byte BE:', ethers.zeroPadValue(ethers.toBeHex(nonce), 32));
console.log('  Timestamp as 32-byte BE:', ethers.zeroPadValue(ethers.toBeHex(timestamp), 32));