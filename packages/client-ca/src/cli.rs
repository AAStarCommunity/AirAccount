// Licensed to AirAccount under the Apache License, Version 2.0
// CLI utilities and helpers

use anyhow::Result;
use std::io::{self, Write};

/// Print a formatted banner
pub fn print_banner() {
    println!(r#"
╔════════════════════════════════════════════════════════════╗
║                      AirAccount                           ║
║              TEE-based Web3 Wallet System                 ║
║                                                           ║
║  🔒 Hardware-secured private keys                         ║
║  🛡️  TrustZone TEE protection                             ║  
║  🔐 Biometric authentication                              ║
║  🌐 Multi-chain support                                   ║
╚════════════════════════════════════════════════════════════╝
"#);
}

/// Prompt user for confirmation
pub fn confirm_action(message: &str) -> Result<bool> {
    print!("{} (y/N): ", message);
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}

/// Print a success message
pub fn success(message: &str) {
    println!("✅ {}", message);
}

/// Print an error message
pub fn error(message: &str) {
    eprintln!("❌ {}", message);
}

/// Print a warning message
pub fn warning(message: &str) {
    println!("⚠️  {}", message);
}

/// Print an info message
pub fn info(message: &str) {
    println!("ℹ️  {}", message);
}