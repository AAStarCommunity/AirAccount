// TA Client - All key operations MUST be done in TA
use crate::types::*;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use chrono::Utc;
use optee_teec::{Context, Session, Operation, ParamType, ParamTmpRef, ParamValue};

// eth_wallet TA UUID (from uuid.txt)
const ETH_WALLET_TA_UUID: &str = include_str!("../../../../third_party/incubator-teaclave-trustzone-sdk/projects/web3/eth_wallet/uuid.txt");

pub struct TAKmsService {
    accounts: Arc<Mutex<HashMap<String, StoredAccount>>>,
    region: String,
    account_id: String,
    context: Arc<Mutex<Option<Context>>>,
}

impl TAKmsService {
    pub fn new(region: String, account_id: String) -> Self {
        Self {
            accounts: Arc::new(Mutex::new(HashMap::new())),
            region,
            account_id,
            context: Arc::new(Mutex::new(None)),
        }
    }

    fn get_context(&self) -> Result<Context> {
        let mut ctx_guard = self.context.lock().unwrap();
        if ctx_guard.is_none() {
            let context = Context::new()?;
            *ctx_guard = Some(context);
        }
        Ok(ctx_guard.as_ref().unwrap().clone())
    }

    fn call_ta<I, O>(&self, command: TACommand, input: &I) -> Result<O>
    where
        I: serde::Serialize,
        O: serde::de::DeserializeOwned,
    {
        let context = self.get_context()?;

        // Parse TA UUID
        let uuid = Uuid::parse_str(ETH_WALLET_TA_UUID.trim())
            .map_err(|e| anyhow!("Invalid TA UUID: {}", e))?;

        // Open session with eth_wallet TA
        let session = Session::open(&context, &uuid, 0, None, None)?;

        // Serialize input
        let input_data = bincode::serialize(input)?;
        let mut output_buffer = vec![0u8; 4096]; // Allocate output buffer
        let mut _error_buffer = vec![0u8; 1024];  // Allocate error buffer

        // Prepare operation parameters
        let param_types = [ParamType::MemrefInout, ParamType::MemrefInout, ParamType::Value];
        let mut operation = Operation::new(0, &param_types, [
            ParamTmpRef::new_input(&input_data).into(),
            ParamTmpRef::new_output(&mut output_buffer).into(),
            ParamValue::new(0, 0, 0, 0).into(),
        ]);

        // Invoke TA command
        session.invoke_command(command as u32, &mut operation)?;

        // Get output length from parameter 2
        let output_len = operation.parameters()[2].value().unwrap().a() as usize;
        if output_len == 0 {
            return Err(anyhow!("TA returned empty output"));
        }

        // Deserialize output
        let output_data = &output_buffer[..output_len];
        let output: O = bincode::deserialize(output_data)?;

        Ok(output)
    }

    pub async fn create_account(&self, request: CreateAccountRequest) -> Result<CreateAccountResponse> {
        // Call eth_wallet TA CreateWallet
        let ta_input = TACreateWalletInput {};
        let ta_output: TACreateWalletOutput = self.call_ta(TACommand::CreateWallet, &ta_input)?;

        // Generate ARN and metadata
        let account_id = ta_output.wallet_id.to_string();
        let arn = format!(
            "arn:aws:kms:{}:{}:account/{}",
            self.region, self.account_id, account_id
        );

        let metadata = AccountMetadata {
            account_id: account_id.clone(),
            arn: arn.clone(),
            creation_date: Utc::now(),
            enabled: true,
            description: request.description.unwrap_or_else(|| "TA HD Account".to_string()),
            wallet_type: "HD_BIP32".to_string(),
            has_mnemonic: true,
        };

        // Store account metadata (non-sensitive)
        let stored_account = StoredAccount {
            wallet_id: ta_output.wallet_id,
            metadata: metadata.clone(),
        };

        let mut accounts = self.accounts.lock().unwrap();
        accounts.insert(account_id, stored_account);

        Ok(CreateAccountResponse {
            account_metadata: metadata,
            mnemonic: ta_output.mnemonic,
        })
    }

    pub async fn describe_account(&self, request: DescribeAccountRequest) -> Result<DescribeAccountResponse> {
        let accounts = self.accounts.lock().unwrap();
        let stored_account = accounts.get(&request.account_id)
            .ok_or_else(|| anyhow!("Account not found: {}", request.account_id))?;

        Ok(DescribeAccountResponse {
            account_metadata: stored_account.metadata.clone(),
        })
    }

    pub async fn list_accounts(&self) -> Result<Vec<AccountMetadata>> {
        let accounts = self.accounts.lock().unwrap();
        Ok(accounts.values().map(|a| a.metadata.clone()).collect())
    }

    pub async fn derive_address(&self, request: DeriveAddressRequest) -> Result<DeriveAddressResponse> {
        // Get wallet_id from stored account
        let accounts = self.accounts.lock().unwrap();
        let stored_account = accounts.get(&request.account_id)
            .ok_or_else(|| anyhow!("Account not found: {}", request.account_id))?;

        // Call eth_wallet TA DeriveAddress
        let ta_input = TADeriveAddressInput {
            wallet_id: stored_account.wallet_id,
            hd_path: request.derivation_path.clone(),
        };

        let ta_output: TADeriveAddressOutput = self.call_ta(TACommand::DeriveAddress, &ta_input)?;

        // Convert TA response to API response
        let address = format!("0x{}", hex::encode(ta_output.address));
        let public_key = base64::engine::general_purpose::STANDARD.encode(&ta_output.public_key);

        Ok(DeriveAddressResponse {
            account_id: request.account_id,
            address,
            derivation_path: request.derivation_path,
            public_key,
        })
    }

    pub async fn sign_transaction(&self, request: SignTransactionRequest) -> Result<SignTransactionResponse> {
        // Get wallet_id from stored account
        let accounts = self.accounts.lock().unwrap();
        let stored_account = accounts.get(&request.account_id)
            .ok_or_else(|| anyhow!("Account not found: {}", request.account_id))?;

        // Convert API transaction to TA transaction
        let ta_transaction = TAEthTransaction::from(request.transaction);

        // Call eth_wallet TA SignTransaction
        let ta_input = TASignTransactionInput {
            wallet_id: stored_account.wallet_id,
            hd_path: request.derivation_path.clone(),
            transaction: ta_transaction,
        };

        let ta_output: TASignTransactionOutput = self.call_ta(TACommand::SignTransaction, &ta_input)?;

        // Convert TA signature to API response
        let signature = base64::engine::general_purpose::STANDARD.encode(&ta_output.signature);

        // Create mock transaction hash and raw transaction for now
        // In a real implementation, these would be computed from the signed transaction
        let transaction_hash = format!("0x{}", hex::encode(&ta_output.signature[..32]));
        let raw_transaction = format!("0x{}", hex::encode(&ta_output.signature));

        Ok(SignTransactionResponse {
            account_id: request.account_id,
            signature,
            transaction_hash,
            raw_transaction,
        })
    }

    pub async fn remove_account(&self, request: RemoveAccountRequest) -> Result<RemoveAccountResponse> {
        // Get wallet_id from stored account
        let accounts = self.accounts.lock().unwrap();
        let stored_account = accounts.get(&request.account_id)
            .ok_or_else(|| anyhow!("Account not found: {}", request.account_id))?;

        let wallet_id = stored_account.wallet_id;
        drop(accounts); // Release lock before TA call

        // Call eth_wallet TA RemoveWallet
        let ta_input = TARemoveWalletInput { wallet_id };
        let _ta_output: TARemoveWalletOutput = self.call_ta(TACommand::RemoveWallet, &ta_input)?;

        // Remove from local storage
        let mut accounts = self.accounts.lock().unwrap();
        accounts.remove(&request.account_id);

        Ok(RemoveAccountResponse {
            account_id: request.account_id,
            removed: true,
        })
    }
}