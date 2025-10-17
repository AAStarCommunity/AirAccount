// 真实的 eth_wallet TA 集成
use anyhow::{bail, Result};
use optee_teec::{Context, Operation, ParamType, Uuid};
use optee_teec::{ParamNone, ParamTmpRef, ParamValue};
use uuid::Uuid as UuidType;
use bincode;
use hex;

const OUTPUT_MAX_SIZE: usize = 1024;

// 从 eth_wallet 项目复制的常量和数据结构
pub const ETH_WALLET_UUID: &str = "70e328e2-8bca-4bb9-a5be-e7e639b97ec0";

#[derive(num_enum::FromPrimitive, num_enum::IntoPrimitive, Debug)]
#[repr(u32)]
pub enum Command {
    CreateWallet,
    RemoveWallet,
    DeriveAddress,
    SignTransaction,
    #[default]
    Unknown,
}

// 输入输出数据结构 - 需要与 eth_wallet TA 保持一致
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CreateWalletOutput {
    pub wallet_id: UuidType,
    pub mnemonic: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct RemoveWalletInput {
    pub wallet_id: UuidType,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DeriveAddressInput {
    pub wallet_id: UuidType,
    pub hd_path: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DeriveAddressOutput {
    pub address: [u8; 20],
    pub public_key: Vec<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct EthTransaction {
    pub chain_id: u64,
    pub nonce: u128,
    pub to: Option<[u8; 20]>,
    pub value: u128,
    pub gas_price: u128,
    pub gas: u128,
    pub data: Vec<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SignTransactionInput {
    pub wallet_id: UuidType,
    pub hd_path: String,
    pub transaction: EthTransaction,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SignTransactionOutput {
    pub signature: Vec<u8>,
}

// ========================================
// 真实TA实现 - 连接到eth_wallet TA
// ========================================

pub struct RealEthWalletTA;

impl RealEthWalletTA {
    pub fn new() -> Self {
        Self
    }

    /// 调用 TA 命令的通用函数
    fn invoke_command(&self, command: Command, input: &[u8]) -> optee_teec::Result<Vec<u8>> {
        let mut ctx = Context::new()?;
        let uuid = Uuid::parse_str(ETH_WALLET_UUID)
            .map_err(|_| optee_teec::Error::new(optee_teec::ErrorKind::ItemNotFound))?;
        let mut session = ctx.open_session(uuid)?;

        println!("🔐 TA调用: 命令={:?}", command);

        // 输入缓冲区
        let p0 = ParamTmpRef::new_input(input);
        // 输出缓冲区
        let mut output = vec![0u8; OUTPUT_MAX_SIZE];
        let p1 = ParamTmpRef::new_output(output.as_mut_slice());
        // 输出缓冲区大小
        let p2 = ParamValue::new(0, 0, ParamType::ValueInout);

        let mut operation = Operation::new(0, p0, p1, p2, ParamNone);
        match session.invoke_command(command as u32, &mut operation) {
            Ok(()) => {
                println!("✅ TA调用成功");
                let output_len = operation.parameters().2.a() as usize;
                Ok(output[..output_len].to_vec())
            }
            Err(e) => {
                let output_len = operation.parameters().2.a() as usize;
                let err_message = String::from_utf8_lossy(&output[..output_len]);
                println!("❌ TA调用失败: {:?}", err_message);
                Err(e)
            }
        }
    }

    /// 调用 TA CreateWallet 命令
    pub async fn create_wallet(&self) -> Result<(UuidType, String)> {
        println!("🔐 TA CreateWallet: 在TEE中生成真实HD钱包");

        let serialized_output = self.invoke_command(Command::CreateWallet, &[])?;
        let output: CreateWalletOutput = bincode::deserialize(&serialized_output)?;

        println!("✅ TA返回真实钱包: ID={}", output.wallet_id);
        Ok((output.wallet_id, output.mnemonic))
    }

    /// 调用 TA RemoveWallet 命令
    pub async fn remove_wallet(&self, wallet_id: &str) -> Result<()> {
        println!("🔐 TA RemoveWallet: 在TEE中删除钱包 {}", wallet_id);

        let uuid = UuidType::parse_str(wallet_id)?;
        let input = RemoveWalletInput { wallet_id: uuid };
        let _output = self.invoke_command(Command::RemoveWallet, &bincode::serialize(&input)?)?;

        println!("✅ TA成功删除钱包: {}", wallet_id);
        Ok(())
    }

    /// 调用 TA DeriveAddress 命令
    pub async fn derive_address(&self, wallet_id: &str, path: &str) -> Result<(String, String)> {
        println!("🔐 TA DeriveAddress: 在TEE中派生地址 {} {}", wallet_id, path);

        let uuid = UuidType::parse_str(wallet_id)?;
        let input = DeriveAddressInput {
            wallet_id: uuid,
            hd_path: path.to_string(),
        };

        let serialized_output = self.invoke_command(Command::DeriveAddress, &bincode::serialize(&input)?)?;
        let output: DeriveAddressOutput = bincode::deserialize(&serialized_output)?;

        let address = format!("0x{}", hex::encode(&output.address));
        let public_key = hex::encode(&output.public_key);

        println!("✅ TA返回真实地址: {} 公钥: {}...", address, &public_key[..20]);
        Ok((address, public_key))
    }

    /// 调用 TA SignTransaction 命令
    pub async fn sign_transaction(
        &self,
        wallet_id: &str,
        path: &str,
        tx: &crate::api::EthereumTransaction
    ) -> Result<(String, String)> {
        println!("🔐 TA SignTransaction: 在TEE中签名交易 {} {}", wallet_id, path);

        let uuid = UuidType::parse_str(wallet_id)?;

        // 转换地址字符串为字节数组
        let to_bytes = if tx.to.starts_with("0x") {
            let hex_str = &tx.to[2..];
            if hex_str.len() != 40 {
                bail!("Invalid address length");
            }
            let mut bytes = [0u8; 20];
            hex::decode_to_slice(hex_str, &mut bytes)?;
            Some(bytes)
        } else {
            None
        };

        let transaction = EthTransaction {
            chain_id: tx.chain_id,
            nonce: tx.nonce as u128,
            to: to_bytes,
            value: tx.value.parse::<u128>().unwrap_or(0),
            gas_price: tx.gas_price.parse::<u128>().unwrap_or(0),
            gas: tx.gas as u128,
            data: hex::decode(&tx.data.trim_start_matches("0x")).unwrap_or_default(),
        };

        let input = SignTransactionInput {
            wallet_id: uuid,
            hd_path: path.to_string(),
            transaction,
        };

        let serialized_output = self.invoke_command(Command::SignTransaction, &bincode::serialize(&input)?)?;
        let output: SignTransactionOutput = bincode::deserialize(&serialized_output)?;

        let signature = hex::encode(&output.signature);
        let tx_hash = format!("0x{}", signature[..64].to_string()); // 简化的交易哈希

        println!("✅ TA返回真实签名: {}...", &signature[..20]);
        Ok((signature, tx_hash))
    }
}

