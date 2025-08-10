// Licensed to AirAccount under the Apache License, Version 2.0
// AirAccount Client Application - Main Entry Point

use clap::{Args, Parser, Subcommand};
use anyhow::{Result, Context};
use log::{info, error};
use uuid::Uuid;

mod tee_client;

#[cfg(feature = "mock_tee")]
mod mock_tee;

use tee_client::TeeClient;
use airaccount_proto::{WalletCommand, WalletResponse};

#[derive(Parser)]
#[command(name = "airaccount-ca")]
#[command(about = "AirAccount Client Application - TEE-based Web3 Wallet")]
#[command(version = "0.1.0")]
struct AirAccountCli {
    #[command(subcommand)]
    command: Commands,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new wallet in TEE
    CreateWallet,
    
    /// Derive an address from an existing wallet
    DeriveAddress(DeriveAddressArgs),
    
    /// Sign a transaction using TEE wallet
    SignTransaction(SignTransactionArgs),
    
    /// Remove a wallet from TEE storage
    RemoveWallet(RemoveWalletArgs),
    
    /// Run interactive tests
    Test,
}

#[derive(Args)]
struct DeriveAddressArgs {
    /// Wallet ID
    #[arg(short = 'w', long)]
    wallet_id: String,
}

#[derive(Args)]
struct SignTransactionArgs {
    /// Wallet ID
    #[arg(short = 'w', long)]
    wallet_id: String,
    
    /// Transaction data (hex encoded)
    #[arg(short = 't', long)]
    transaction: String,
}

#[derive(Args)]
struct RemoveWalletArgs {
    /// Wallet ID
    #[arg(short = 'w', long)]
    wallet_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = AirAccountCli::parse();
    
    // Initialize logging
    if cli.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }
    
    info!("AirAccount Client Application starting");
    
    // Initialize TEE client
    let mut tee_client = TeeClient::new()
        .context("Failed to initialize TEE client")?;
    
    // Execute command
    match cli.command {
        Commands::CreateWallet => {
            create_wallet(&mut tee_client).await?;
        }
        Commands::DeriveAddress(args) => {
            derive_address(&mut tee_client, &args.wallet_id).await?;
        }
        Commands::SignTransaction(args) => {
            let tx_data = hex::decode(&args.transaction)
                .context("Invalid transaction hex data")?;
            sign_transaction(&mut tee_client, &args.wallet_id, &tx_data).await?;
        }
        Commands::RemoveWallet(args) => {
            remove_wallet(&mut tee_client, &args.wallet_id).await?;
        }
        Commands::Test => {
            run_tests(&mut tee_client).await?;
        }
    }
    
    info!("AirAccount Client Application finished");
    Ok(())
}

async fn create_wallet(tee_client: &mut TeeClient) -> Result<()> {
    println!("CA: Creating new wallet...");
    
    let response = tee_client.send_command(WalletCommand::CreateWallet).await?;
    
    match response {
        WalletResponse::CreateWallet(resp) => {
            if resp.success {
                println!("✅ Wallet created successfully!");
                if let Some(wallet_id) = resp.wallet_id {
                    println!("Wallet ID: {}", wallet_id);
                }
                if let Some(mnemonic) = resp.mnemonic {
                    println!("🔐 Mnemonic (backup safely): {}", mnemonic);
                    println!("⚠️  WARNING: Store this mnemonic phrase securely!");
                }
            } else {
                println!("❌ Failed to create wallet");
                if let Some(error) = resp.error {
                    println!("Error: {}", error);
                }
            }
        }
        _ => {
            return Err(anyhow::anyhow!("Unexpected response type"));
        }
    }
    
    Ok(())
}

async fn derive_address(tee_client: &mut TeeClient, wallet_id: &str) -> Result<()> {
    println!("CA: Deriving address for wallet: {}", wallet_id);
    
    let response = tee_client.send_command(WalletCommand::DeriveAddress {
        wallet_id: wallet_id.to_string(),
    }).await?;
    
    match response {
        WalletResponse::DeriveAddress(resp) => {
            if resp.success {
                println!("✅ Address derived successfully!");
                if let Some(address) = resp.address {
                    println!("Address: {}", address);
                }
                if let Some(public_key) = resp.public_key {
                    println!("Public Key: {}", public_key);
                }
            } else {
                println!("❌ Failed to derive address");
                if let Some(error) = resp.error {
                    println!("Error: {}", error);
                }
            }
        }
        _ => {
            return Err(anyhow::anyhow!("Unexpected response type"));
        }
    }
    
    Ok(())
}

async fn sign_transaction(tee_client: &mut TeeClient, wallet_id: &str, tx_data: &[u8]) -> Result<()> {
    println!("CA: Signing transaction for wallet: {}", wallet_id);
    println!("Transaction data: {} bytes", tx_data.len());
    
    let response = tee_client.send_command(WalletCommand::SignTransaction {
        wallet_id: wallet_id.to_string(),
        transaction_data: tx_data.to_vec(),
    }).await?;
    
    match response {
        WalletResponse::SignTransaction(resp) => {
            if resp.success {
                println!("✅ Transaction signed successfully!");
                if let Some(signature) = resp.signature {
                    println!("Signature: {}", signature);
                }
            } else {
                println!("❌ Failed to sign transaction");
                if let Some(error) = resp.error {
                    println!("Error: {}", error);
                }
            }
        }
        _ => {
            return Err(anyhow::anyhow!("Unexpected response type"));
        }
    }
    
    Ok(())
}

async fn remove_wallet(tee_client: &mut TeeClient, wallet_id: &str) -> Result<()> {
    println!("CA: Removing wallet: {}", wallet_id);
    
    let response = tee_client.send_command(WalletCommand::RemoveWallet {
        wallet_id: wallet_id.to_string(),
    }).await?;
    
    match response {
        WalletResponse::RemoveWallet(resp) => {
            if resp.success {
                println!("✅ Wallet removed successfully!");
            } else {
                println!("❌ Failed to remove wallet");
                if let Some(error) = resp.error {
                    println!("Error: {}", error);
                }
            }
        }
        _ => {
            return Err(anyhow::anyhow!("Unexpected response type"));
        }
    }
    
    Ok(())
}

async fn run_tests(tee_client: &mut TeeClient) -> Result<()> {
    println!("🧪 Running AirAccount integration tests...");
    
    // Test 1: Create wallet
    println!("\n1️⃣  Testing wallet creation...");
    let response = tee_client.send_command(WalletCommand::CreateWallet).await?;
    
    let wallet_id = match response {
        WalletResponse::CreateWallet(resp) => {
            if resp.success {
                println!("✅ Wallet creation: PASS");
                resp.wallet_id.unwrap()
            } else {
                println!("❌ Wallet creation: FAIL");
                return Err(anyhow::anyhow!("Wallet creation failed"));
            }
        }
        _ => return Err(anyhow::anyhow!("Unexpected response")),
    };
    
    // Test 2: Derive address
    println!("\n2️⃣  Testing address derivation...");
    let response = tee_client.send_command(WalletCommand::DeriveAddress {
        wallet_id: wallet_id.clone(),
    }).await?;
    
    match response {
        WalletResponse::DeriveAddress(resp) => {
            if resp.success {
                println!("✅ Address derivation: PASS");
                if let Some(address) = resp.address {
                    println!("   Address: {}", address);
                }
            } else {
                println!("❌ Address derivation: FAIL");
                return Err(anyhow::anyhow!("Address derivation failed"));
            }
        }
        _ => return Err(anyhow::anyhow!("Unexpected response")),
    }
    
    // Test 3: Sign dummy transaction
    println!("\n3️⃣  Testing transaction signing...");
    let dummy_tx = vec![0x12, 0x34, 0x56, 0x78]; // Dummy transaction data
    let response = tee_client.send_command(WalletCommand::SignTransaction {
        wallet_id: wallet_id.clone(),
        transaction_data: dummy_tx,
    }).await?;
    
    match response {
        WalletResponse::SignTransaction(resp) => {
            if resp.success {
                println!("✅ Transaction signing: PASS");
            } else {
                println!("❌ Transaction signing: FAIL");
                return Err(anyhow::anyhow!("Transaction signing failed"));
            }
        }
        _ => return Err(anyhow::anyhow!("Unexpected response")),
    }
    
    // Test 4: Remove wallet
    println!("\n4️⃣  Testing wallet removal...");
    let response = tee_client.send_command(WalletCommand::RemoveWallet {
        wallet_id: wallet_id.clone(),
    }).await?;
    
    match response {
        WalletResponse::RemoveWallet(resp) => {
            if resp.success {
                println!("✅ Wallet removal: PASS");
            } else {
                println!("❌ Wallet removal: FAIL");
                return Err(anyhow::anyhow!("Wallet removal failed"));
            }
        }
        _ => return Err(anyhow::anyhow!("Unexpected response")),
    }
    
    println!("\n🎉 All tests passed! AirAccount TEE integration is working correctly.");
    Ok(())
}