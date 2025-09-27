use clap::{Parser, Subcommand};
use kms_core::{KeyAlgorithm, KeyId, KmsOperation};

#[derive(Parser)]
#[command(name = "kms-cli")]
#[command(about = "TEE-based Key Management System CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new key pair
    Generate {
        /// Key identifier (hex string)
        #[arg(short, long)]
        key_id: String,
        /// Key algorithm
        #[arg(short, long, default_value = "secp256k1")]
        algorithm: String,
    },
    /// Get public key
    GetPubKey {
        /// Key identifier (hex string)
        #[arg(short, long)]
        key_id: String,
    },
    /// Sign a message
    Sign {
        /// Key identifier (hex string)
        #[arg(short, long)]
        key_id: String,
        /// Message to sign (hex string)
        #[arg(short, long)]
        message: String,
    },
    /// Delete a key
    Delete {
        /// Key identifier (hex string)
        #[arg(short, long)]
        key_id: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { key_id, algorithm } => {
            let key_id = parse_key_id(&key_id)?;
            let algorithm = parse_algorithm(&algorithm)?;
            println!("Generating key with ID: {:?}, Algorithm: {:?}", key_id, algorithm);
            // TODO: Call TA through optee-teec
        }
        Commands::GetPubKey { key_id } => {
            let key_id = parse_key_id(&key_id)?;
            println!("Getting public key for ID: {:?}", key_id);
            // TODO: Call TA through optee-teec
        }
        Commands::Sign { key_id, message } => {
            let key_id = parse_key_id(&key_id)?;
            let message_bytes = hex::decode(&message)?;
            println!("Signing message with key ID: {:?}", key_id);
            // TODO: Call TA through optee-teec
        }
        Commands::Delete { key_id } => {
            let key_id = parse_key_id(&key_id)?;
            println!("Deleting key with ID: {:?}", key_id);
            // TODO: Call TA through optee-teec
        }
    }

    Ok(())
}

fn parse_key_id(key_id_str: &str) -> anyhow::Result<KeyId> {
    let bytes = hex::decode(key_id_str)?;
    if bytes.len() != 32 {
        anyhow::bail!("Key ID must be 32 bytes (64 hex characters)");
    }
    let mut key_id = [0u8; 32];
    key_id.copy_from_slice(&bytes);
    Ok(key_id)
}

fn parse_algorithm(algorithm_str: &str) -> anyhow::Result<KeyAlgorithm> {
    match algorithm_str.to_lowercase().as_str() {
        "secp256k1" => Ok(KeyAlgorithm::Secp256k1),
        "ed25519" => Ok(KeyAlgorithm::Ed25519),
        _ => anyhow::bail!("Unsupported algorithm: {}", algorithm_str),
    }
}