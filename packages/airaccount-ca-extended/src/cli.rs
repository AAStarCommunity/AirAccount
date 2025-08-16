/**
 * AirAccount CA Extended - CLI工具
 * 基于现有airaccount-ca的CLI功能扩展
 */

use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::{info, error};

mod tee_client;
use tee_client::TeeClient;

#[derive(Parser)]
#[command(name = "airaccount-ca-extended")]
#[command(about = "AirAccount CA Extended - TEE-based wallet CLI with WebAuthn support")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Test TEE connection
    Test,
    /// Create a new wallet with Passkey
    CreateWallet {
        /// Email address
        #[arg(short, long)]
        email: String,
        /// Passkey credential ID
        #[arg(short, long)]
        credential_id: String,
        /// Passkey public key (base64 encoded)
        #[arg(short, long)]
        public_key: String,
    },
    /// Derive address for a wallet
    DeriveAddress {
        /// Wallet ID
        wallet_id: u32,
    },
    /// Sign a transaction
    SignTransaction {
        /// Wallet ID
        wallet_id: u32,
        /// Transaction data
        #[arg(short, long)]
        transaction: String,
    },
    /// Get wallet information
    GetWalletInfo {
        /// Wallet ID
        wallet_id: u32,
    },
    /// List all wallets
    ListWallets,
    /// Test security features
    TestSecurity,
    /// Start HTTP API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3001")]
        port: u16,
    },
}

fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("airaccount_ca_extended=info")
        .init();

    let cli = Cli::parse();

    // 初始化TEE客户端
    let mut tee_client = TeeClient::new()?;

    match cli.command {
        Commands::Test => {
            info!("🔧 Testing TEE connection...");
            let response = tee_client.test_connection()?;
            println!("✅ TEE Response: {}", response);
        }

        Commands::CreateWallet { email, credential_id, public_key } => {
            info!("🔐 Creating wallet for email: {}", email);
            
            // 解码base64公钥
            let public_key_bytes = base64::decode(&public_key)?;
            
            let result = tee_client.create_account_with_passkey(
                &email,
                &credential_id,
                &public_key_bytes,
            )?;
            
            println!("✅ Wallet created successfully:");
            println!("   Wallet ID: {}", result.wallet_id);
            println!("   Ethereum Address: {}", result.ethereum_address);
            println!("   TEE Device ID: {}", result.tee_device_id);
        }

        Commands::DeriveAddress { wallet_id } => {
            info!("🔑 Deriving address for wallet: {}", wallet_id);
            let response = tee_client.derive_address(wallet_id)?;
            println!("✅ Address Response: {}", response);
        }

        Commands::SignTransaction { wallet_id, transaction } => {
            info!("✍️ Signing transaction for wallet: {}", wallet_id);
            let result = tee_client.sign_transaction(wallet_id, &transaction)?;
            
            println!("✅ Transaction signed successfully:");
            println!("   Transaction Hash: {}", result.transaction_hash);
            println!("   Signature: {}", result.signature);
            println!("   Wallet ID: {}", result.wallet_id);
        }

        Commands::GetWalletInfo { wallet_id } => {
            info!("📊 Getting wallet info for: {}", wallet_id);
            let response = tee_client.get_wallet_info(wallet_id)?;
            println!("✅ Wallet Info: {}", response);
        }

        Commands::ListWallets => {
            info!("📋 Listing all wallets");
            let response = tee_client.list_wallets()?;
            println!("✅ Wallets: {}", response);
        }

        Commands::TestSecurity => {
            info!("🛡️ Testing security features");
            let response = tee_client.test_security()?;
            println!("✅ Security Test: {}", response);
        }

        Commands::Serve { port } => {
            error!("❌ HTTP server mode not available in CLI build");
            error!("   Use 'cargo run --bin ca-server' to start the HTTP server");
            std::process::exit(1);
        }
    }

    Ok(())
}