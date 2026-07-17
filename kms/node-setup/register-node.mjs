#!/usr/bin/env node
/*
 * register-node.mjs — 社区节点一键链上注册(模型 A：预充值 operator + registerWithProof + KMS /pop）
 *
 * 由 setup 向导(setup-server.py step4)或运维手动调用。用 SDK @aastar/operator 的
 * onboardDvtNode 完成"stake + registerWithProof"一键上链。三种 PoP 来源(择一)：
 *   - KMS /pop  popSigner   —— 默认。key-less TEE 节点，BLS 私钥永不出板(推荐)。
 *   - BLS_SECRET_KEY        —— 本地/独立节点(HSM/keystore 解出的 32B scalar)。
 *
 * 模型 A：operator EOA 已被 AAstar 预充值(ETH + 30 GToken),故不传 funderWallet,operator 自注册。
 *
 * 环境变量：
 *   NETWORK            sepolia | mainnet          (默认 sepolia)
 *   ETH_RPC_URL        链 RPC(必填)
 *   OPERATOR_KEY       0x+64hex operator 私钥(实注册必填;/etc/airaccount/dvt-operator.key)
 *   OPERATOR_ADDRESS   只读 --dry-run 时可只给地址(无私钥)
 *   KMS_SIGNER_URL     KMS internal signer(默认 http://127.0.0.1:3100)
 *   KMS_BLS_SIGNER_TOKEN  X-Signer-Token(/pop 鉴权)
 *   BLS_SECRET_KEY     可选:本地 BLS 私钥,替代 KMS /pop
 *   VALIDATOR_ADDRESS  ⚠️必填覆盖:Sepolia 实链 0x539B96…(别用 canonical 漂移的 0x0)
 *   GTOKEN_ADDRESS     ⚠️Sepolia 实链 0x4c09aE57…(别用 canonical 0x8d6Fe002)
 *   STAKING_ADDRESS    ⚠️Sepolia 实链 0x472297B5…
 *
 * 用法：
 *   node register-node.mjs --dry-run    # 只读:算质押/出资计划 + simulate,不上链
 *   node register-node.mjs              # 实注册(需 OPERATOR_KEY + operator 已预充值)
 *
 * 退出码：0 成功/已注册/dry-run OK；1 失败。stdout 是 JSON 结果。
 */
import { createPublicClient, createWalletClient, http, keccak256, isHex } from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { sepolia, mainnet } from 'viem/chains';
import { onboardDvtNode } from '@aastar/operator';

const DRY_RUN = process.argv.includes('--dry-run');
const env = process.env;
const fail = (m) => { console.error(`register-node: ${m}`); process.exit(1); };

const NETWORK = env.NETWORK || 'sepolia';
const chain = NETWORK === 'mainnet' ? mainnet : sepolia;
const rpc = env.ETH_RPC_URL;
if (!rpc) fail('need ETH_RPC_URL');

const publicClient = createPublicClient({ chain, transport: http(rpc) });

// operator wallet: 实注册用私钥(签名);dry-run 可只给地址(只读/simulate)
let operatorWallet;
if (env.OPERATOR_KEY) {
  const key = env.OPERATOR_KEY.trim();
  if (!isHex(key) || key.length !== 66) fail('OPERATOR_KEY 必须是 0x+64hex');
  operatorWallet = createWalletClient({ account: privateKeyToAccount(key), chain, transport: http(rpc) });
} else if (DRY_RUN && env.OPERATOR_ADDRESS) {
  operatorWallet = createWalletClient({ account: env.OPERATOR_ADDRESS.trim(), chain, transport: http(rpc) });
} else {
  fail('need OPERATOR_KEY (实注册) 或 OPERATOR_ADDRESS (--dry-run 只读)');
}

// PoP 来源:BLS_SECRET_KEY(本地) 或 KMS /pop popSigner(key-less TEE)
let popArgs;
if (env.BLS_SECRET_KEY) {
  popArgs = { blsSecretKey: env.BLS_SECRET_KEY.trim() };
} else {
  const signerUrl = env.KMS_SIGNER_URL || 'http://127.0.0.1:3100';
  popArgs = {
    popSigner: async () => {
      const r = await fetch(`${signerUrl}/pop`, {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          ...(env.KMS_BLS_SIGNER_TOKEN ? { 'X-Signer-Token': env.KMS_BLS_SIGNER_TOKEN } : {}),
        },
        body: JSON.stringify({}), // handler 忽略 body;key_id 从 KMS 的 env KMS_BLS_KEY_ID 取
      });
      if (!r.ok) throw new Error(`KMS /pop ${r.status}: ${(await r.text()).slice(0, 200)}`);
      const d = await r.json(); // { public_key, pop_point, pop_signature }
      if (!d.public_key || !d.pop_point || !d.pop_signature) {
        throw new Error('KMS /pop 响应缺字段(public_key/pop_point/pop_signature)');
      }
      return {
        publicKey: d.public_key,
        popPoint: d.pop_point,
        popSig: d.pop_signature,
        nodeId: keccak256(d.public_key), // 合约按此绑定;SDK 也会自算,这里对齐
      };
    },
  };
}

// 地址:显式覆盖 canonical(⚠️Sepolia 必须传实链值,canonical 有漂移)
const addrs = {};
if (env.VALIDATOR_ADDRESS) addrs.validator = env.VALIDATOR_ADDRESS.trim();
if (env.GTOKEN_ADDRESS) addrs.gToken = env.GTOKEN_ADDRESS.trim();
if (env.STAKING_ADDRESS) addrs.staking = env.STAKING_ADDRESS.trim();

const jsonBig = (o) => JSON.stringify(o, (_k, v) => (typeof v === 'bigint' ? v.toString() : v), 2);

try {
  const res = await onboardDvtNode({ publicClient, operatorWallet, ...popArgs, ...addrs, dryRun: DRY_RUN });
  console.log(jsonBig(res));
  process.exit(0);
} catch (e) {
  fail(e?.shortMessage || e?.message || String(e));
}
