// Export private key CLI tool (admin only, no passkey required)
// WARNING: This tool exports private keys in plain text.

use anyhow::Result;
use kms::ta_client::TaClient;
use std::env;
use uuid::Uuid;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.len() > 3 {
        eprintln!("Usage: {} <wallet_id> [derivation_path]", args[0]);
        eprintln!("Default derivation path: m/44'/60'/0'/0/0");
        std::process::exit(1);
    }

    let wallet_id = Uuid::parse_str(&args[1])?;
    let derivation_path = args.get(2).map(|s| s.as_str()).unwrap_or("m/44'/60'/0'/0/0");

    println!("🔑 Exporting private key...");
    println!("   Wallet ID: {}", wallet_id);
    println!("   Derivation Path: {}", derivation_path);
    println!();

    let mut ta_client = TaClient::new()?;
    let private_key = ta_client.export_private_key(wallet_id, derivation_path, None)?;

    println!("✅ Private Key (hex):");
    println!("   0x{}", hex::encode(&private_key));
    println!();
    println!("⚠️  WARNING: Keep this private key secure! Never share it!");

    Ok(())
}
