// Export private key CLI tool
// WARNING: This tool exports private keys in plain text. Use only for debugging/verification.
// Only run this inside QEMU (not in production)

use anyhow::Result;
use kms::ta_client::TaClient;
use std::env;
use uuid::Uuid;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <wallet_id> <derivation_path>", args[0]);
        eprintln!("Example: {} c9ff2117-2fe4-4f6e-8c3a-a197cf74ad07 \"m/44'/60'/0'/0/0\"", args[0]);
        std::process::exit(1);
    }

    let wallet_id = Uuid::parse_str(&args[1])?;
    let derivation_path = &args[2];

    println!("🔑 Exporting private key...");
    println!("   Wallet ID: {}", wallet_id);
    println!("   Derivation Path: {}", derivation_path);
    println!();

    let mut ta_client = TaClient::new()?;
    let private_key = ta_client.export_private_key(wallet_id, derivation_path)?;

    println!("✅ Private Key (hex):");
    println!("   0x{}", hex::encode(&private_key));
    println!();
    println!("⚠️  WARNING: Keep this private key secure! Never share it!");

    Ok(())
}
