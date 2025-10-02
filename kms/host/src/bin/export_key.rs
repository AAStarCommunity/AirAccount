// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! KMS Export Private Key Tool
//!
//! ⚠️  SECURITY WARNING: This tool exports raw private keys from TEE
//! Only use for migration, backup, or debugging purposes
//!
//! Usage:
//!   ./export_key <wallet_id> <derivation_path>
//!   ./export_key <wallet_id> <address>  (TODO: requires Address Cache)
//!
//! Examples:
//!   ./export_key 550e8400-e29b-41d4-a716-446655440000 "m/44'/60'/0'/0/0"
//!   ./export_key 550e8400-e29b-41d4-a716-446655440000 0xe8c78126b210eba23efcd85c5aa0829a3299fa6b  (future)

use anyhow::{anyhow, Result};
use kms::ta_client::TaClient;
use uuid::Uuid;

fn main() -> Result<()> {
    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <wallet_id> <derivation_path_or_address>", args[0]);
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  {} 550e8400-e29b-41d4-a716-446655440000 \"m/44'/60'/0'/0/0\"", args[0]);
        eprintln!("  {} 550e8400-e29b-41d4-a716-446655440000 0xe8c...  (requires Address Cache)", args[0]);
        std::process::exit(1);
    }

    let wallet_id_str = &args[1];
    let path_or_address = &args[2];

    // Parse wallet UUID
    let wallet_id = Uuid::parse_str(wallet_id_str)
        .map_err(|_| anyhow!("Invalid wallet UUID: {}", wallet_id_str))?;

    // Determine if input is address (0x prefix) or derivation path (m/ prefix)
    let hd_path = if path_or_address.starts_with("0x") {
        // TODO: Implement Address Cache lookup
        // For now, return error with helpful message
        return Err(anyhow!(
            "Address-based export not yet implemented.\n\
             Address Cache system is required to map Address → (wallet_id, derivation_path).\n\
             Please use derivation path directly: export_key {} \"m/44'/60'/0'/0/0\"",
            wallet_id
        ));
    } else if path_or_address.starts_with("m/") {
        path_or_address.clone()
    } else {
        return Err(anyhow!(
            "Invalid derivation path or address. Must start with 'm/' or '0x'\n\
             Got: {}",
            path_or_address
        ));
    };

    println!("🔐 Exporting Private Key");
    println!("   Wallet ID: {}", wallet_id);
    println!("   HD Path:   {}", hd_path);
    println!();

    // Call TA to export private key
    let mut ta_client = TaClient::new()?;
    let private_key = ta_client.export_private_key(wallet_id, &hd_path)?;

    // Display private key
    println!("✅ Private Key Exported Successfully");
    println!();
    println!("Private Key (hex, 32 bytes):");
    println!("{}", hex::encode(&private_key));
    println!();
    println!("⚠️  WARNING: Keep this private key secure!");
    println!("   Anyone with this key can control your funds.");
    println!("   Never share it or store it in plaintext.");

    Ok(())
}
