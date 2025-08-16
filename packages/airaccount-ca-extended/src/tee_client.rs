/**
 * TEE客户端 - 基于现有airaccount-ta-simple的扩展
 */

use anyhow::{anyhow, Result};
use optee_teec::{Context, Operation, Session, Uuid};
use optee_teec::{ParamNone, ParamTmpRef};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

// 使用与airaccount-ta-simple相同的UUID
const AIRACCOUNT_TA_UUID: &str = "11223344-5566-7788-99aa-bbccddeeff01";

// 命令常量 - 与airaccount-ta-simple保持一致
const CMD_HELLO_WORLD: u32 = 0;
const CMD_ECHO: u32 = 1;
const CMD_GET_VERSION: u32 = 2;

// 钱包命令
const CMD_CREATE_WALLET: u32 = 10;
const CMD_REMOVE_WALLET: u32 = 11;
const CMD_DERIVE_ADDRESS: u32 = 12;
const CMD_SIGN_TRANSACTION: u32 = 13;
const CMD_GET_WALLET_INFO: u32 = 14;
const CMD_LIST_WALLETS: u32 = 15;
const CMD_TEST_SECURITY: u32 = 16;

// 扩展命令 - WebAuthn集成
const CMD_CREATE_ACCOUNT_WITH_PASSKEY: u32 = 20;
const CMD_VERIFY_PASSKEY: u32 = 21;
const CMD_SIGN_WITH_PASSKEY: u32 = 22;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeAccountResult {
    pub wallet_id: u32,
    pub ethereum_address: String,
    pub tee_device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeTransferResult {
    pub transaction_hash: String,
    pub signature: String,
    pub wallet_id: u32,
}

pub struct TeeClient {
    _context: Context,
    session: Session,
}

impl TeeClient {
    pub fn new() -> Result<Self> {
        info!("🔧 Initializing AirAccount TEE Client...");
        
        // 解析TA UUID
        let uuid = Uuid::parse_str(AIRACCOUNT_TA_UUID)
            .map_err(|e| anyhow!("Failed to parse TA UUID: {}", e))?;
        
        // 初始化TEE上下文
        let mut context = Context::new()
            .map_err(|e| anyhow!("Failed to create TEE context: {:?}", e))?;
        
        info!("✅ TEE Context created successfully");
        
        // 打开与TA的会话
        let session = context.open_session(uuid)
            .map_err(|e| anyhow!("Failed to open session with AirAccount TA: {:?}", e))?;
        
        info!("✅ Session opened with AirAccount TA (UUID: {})", AIRACCOUNT_TA_UUID);
        
        Ok(TeeClient {
            _context: context,
            session,
        })
    }

    /// 测试TA连接
    pub fn test_connection(&mut self) -> Result<String> {
        info!("📞 Testing TA connection...");
        
        let response = self.invoke_simple_command(CMD_HELLO_WORLD, None)?;
        
        if response.contains("AirAccount") {
            info!("✅ TA connection test successful");
            Ok(response)
        } else {
            error!("❌ Unexpected TA response: {}", response);
            Err(anyhow!("TA connection test failed"))
        }
    }

    /// 创建账户（集成Passkey数据）
    pub fn create_account_with_passkey(
        &mut self,
        email: &str,
        passkey_credential_id: &str,
        passkey_public_key: &[u8],
    ) -> Result<TeeAccountResult> {
        info!("🔐 Creating account with Passkey for: {}", email);

        // 准备输入数据：email + credential_id + public_key
        let mut input_data = Vec::new();
        input_data.extend_from_slice(email.as_bytes());
        input_data.push(0); // 分隔符
        input_data.extend_from_slice(passkey_credential_id.as_bytes());
        input_data.push(0); // 分隔符
        input_data.extend_from_slice(passkey_public_key);

        let response = self.invoke_simple_command(CMD_CREATE_ACCOUNT_WITH_PASSKEY, Some(&input_data))?;
        
        // 解析响应
        if response.starts_with("wallet_created:id=") {
            if let Some(id_str) = response.strip_prefix("wallet_created:id=") {
                if let Ok(wallet_id) = id_str.trim().parse::<u32>() {
                    // 获取地址信息
                    let address_response = self.derive_address(wallet_id)?;
                    let ethereum_address = self.extract_address_from_response(&address_response)?;
                    
                    let result = TeeAccountResult {
                        wallet_id,
                        ethereum_address,
                        tee_device_id: format!("tee_device_{}", wallet_id),
                    };
                    
                    info!("✅ Account created successfully: wallet_id={}, address={}", 
                         wallet_id, result.ethereum_address);
                    
                    return Ok(result);
                }
            }
        }
        
        Err(anyhow!("Failed to parse account creation response: {}", response))
    }

    /// 派生地址
    pub fn derive_address(&mut self, wallet_id: u32) -> Result<String> {
        info!("🔑 Deriving address for wallet: {}", wallet_id);
        
        let input_data = wallet_id.to_le_bytes();
        let response = self.invoke_simple_command(CMD_DERIVE_ADDRESS, Some(&input_data))?;
        
        info!("✅ Address derived for wallet {}", wallet_id);
        Ok(response)
    }

    /// 签名交易
    pub fn sign_transaction(
        &mut self,
        wallet_id: u32,
        transaction_data: &str,
    ) -> Result<TeeTransferResult> {
        info!("✍️ Signing transaction for wallet: {}", wallet_id);

        // 准备交易数据
        let mut input_data = Vec::new();
        input_data.extend_from_slice(&wallet_id.to_le_bytes());
        input_data.extend_from_slice(transaction_data.as_bytes());

        let response = self.invoke_simple_command(CMD_SIGN_TRANSACTION, Some(&input_data))?;
        
        // 解析签名响应
        if response.starts_with("signature:") {
            let signature = response.strip_prefix("signature:").unwrap_or(&response);
            
            // 生成模拟交易哈希
            let transaction_hash = format!("0x{:x}", 
                transaction_data.as_bytes().iter().take(32).fold(0u64, |acc, &b| acc.wrapping_mul(256).wrapping_add(b as u64))
            );
            
            let result = TeeTransferResult {
                transaction_hash,
                signature: signature.to_string(),
                wallet_id,
            };
            
            info!("✅ Transaction signed successfully for wallet {}", wallet_id);
            Ok(result)
        } else {
            Err(anyhow!("Failed to parse signature response: {}", response))
        }
    }

    /// 获取钱包信息
    pub fn get_wallet_info(&mut self, wallet_id: u32) -> Result<String> {
        info!("📊 Getting wallet info for: {}", wallet_id);
        
        let input_data = wallet_id.to_le_bytes();
        let response = self.invoke_simple_command(CMD_GET_WALLET_INFO, Some(&input_data))?;
        
        Ok(response)
    }

    /// 列出所有钱包
    pub fn list_wallets(&mut self) -> Result<String> {
        info!("📋 Listing all wallets");
        
        let response = self.invoke_simple_command(CMD_LIST_WALLETS, None)?;
        Ok(response)
    }

    /// 测试安全功能
    pub fn test_security(&mut self) -> Result<String> {
        info!("🛡️ Testing security features");
        
        let response = self.invoke_simple_command(CMD_TEST_SECURITY, None)?;
        Ok(response)
    }

    // === 私有辅助方法 ===

    fn invoke_simple_command(&mut self, cmd_id: u32, input: Option<&[u8]>) -> Result<String> {
        let mut output_buffer = vec![0u8; 1024];
        let empty_buffer = vec![];
        
        let input_data = input.unwrap_or(&empty_buffer);
        
        let p0 = ParamTmpRef::new_input(input_data);
        let p1 = ParamTmpRef::new_output(output_buffer.as_mut_slice());
        let mut operation = Operation::new(0, p0, p1, ParamNone, ParamNone);
        
        self.session.invoke_command(cmd_id, &mut operation)
            .map_err(|e| anyhow!("TA command {} failed: {:?}", cmd_id, e))?;
        
        // 查找响应的实际长度
        let response_len = output_buffer.iter().position(|&x| x == 0).unwrap_or(output_buffer.len());
        let response = String::from_utf8_lossy(&output_buffer[..response_len]);
        
        Ok(response.to_string())
    }

    fn extract_address_from_response(&self, response: &str) -> Result<String> {
        if let Some(address_hex) = response.strip_prefix("address:") {
            // 转换十六进制字符串为以太坊地址格式
            if address_hex.len() >= 40 {
                let ethereum_address = format!("0x{}", &address_hex[..40]);
                Ok(ethereum_address)
            } else {
                Err(anyhow!("Invalid address format: {}", address_hex))
            }
        } else {
            Err(anyhow!("Address not found in response: {}", response))
        }
    }
}

