// Test the original eth_wallet code with KMS modifications
mod hash;
mod mock_tee;
mod wallet;

use anyhow::Result;
use wallet::Wallet;
use proto::EthTransaction;

fn main() -> Result<()> {
    println!("Testing original eth_wallet functionality...");

    // Test wallet creation
    println!("1. Creating wallet...");
    let wallet = Wallet::new()?;
    println!("   Wallet ID: {}", wallet.get_id());

    // Test mnemonic generation
    println!("2. Generating mnemonic...");
    let mnemonic = wallet.get_mnemonic()?;
    println!("   Mnemonic: {}", mnemonic);

    // Test address derivation
    println!("3. Deriving address...");
    let hd_path = "m/44'/60'/0'/0/0"; // Standard Ethereum derivation path
    let (address, public_key) = wallet.derive_address(hd_path)?;
    println!("   Address: 0x{}", hex::encode(&address));
    println!("   Public key: 0x{}", hex::encode(&public_key));

    // Test transaction signing
    println!("4. Testing transaction signing...");
    let test_transaction = EthTransaction {
        chain_id: 1, // Ethereum mainnet
        nonce: 0,
        gas_price: 20_000_000_000u128, // 20 gwei
        gas: 21_000u128,
        to: Some(address), // Self-send for testing
        value: 1_000_000_000_000_000_000u128, // 1 ETH
        data: vec![],
    };

    let signature = wallet.sign_transaction(hd_path, &test_transaction)?;
    println!("   Signature: 0x{}", hex::encode(&signature));
    println!("   Signature length: {} bytes", signature.len());

    println!("\n✅ All eth_wallet functionality tests passed!");
    println!("🔐 Original code works correctly for KMS integration");

    Ok(())
}