// Basic test to verify our KMS components work
use kms_core::{crypto::CryptoProvider, types::KeyAlgorithm, error::KmsError};

fn main() -> Result<(), KmsError> {
    println!("Testing KMS basic functionality...");

    // Test key generation
    let (private_key, public_key) = CryptoProvider::generate_keypair(KeyAlgorithm::Secp256k1)?;
    println!("Generated key pair:");
    println!("  Private key algorithm: {:?}", private_key.algorithm);
    println!("  Public key algorithm: {:?}", public_key.algorithm);
    println!("  Public key size: {} bytes", public_key.key_data.len());

    // Test signing
    let message = b"Hello, KMS!";
    let signature = CryptoProvider::sign(&private_key, message)?;
    println!("Signature size: {} bytes", signature.len());

    println!("Basic KMS test completed successfully!");
    Ok(())
}