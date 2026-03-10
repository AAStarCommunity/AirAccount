//! CLI tool for managing KMS API keys.
//!
//! Usage:
//!   api-key generate [--label "my-service"]
//!   api-key list
//!   api-key revoke <KEY>

use anyhow::Result;
use kms::db::KmsDb;

fn db_path() -> String {
    std::env::var("KMS_DB_PATH").unwrap_or_else(|_| {
        if std::path::Path::new("/data/kms").exists() {
            "/data/kms/kms.db".to_string()
        } else {
            "kms.db".to_string()
        }
    })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    let db = KmsDb::open(&db_path())?;

    match cmd {
        "generate" => {
            let label = if args.len() > 2 {
                // support: generate --label "xxx" or just generate "xxx"
                if args[2] == "--label" {
                    args.get(3).map(|s| s.as_str()).unwrap_or("")
                } else {
                    args[2].as_str()
                }
            } else {
                ""
            };
            let key = db.generate_api_key(label)?;
            println!("{}", key);
            eprintln!("API key generated. Label: \"{}\"", label);
            eprintln!("Store this key securely — it cannot be retrieved later.");
        }
        "list" => {
            let keys = db.list_api_keys()?;
            if keys.is_empty() {
                println!("No API keys configured.");
            } else {
                println!("{:<40} {:<20} {}", "KEY", "LABEL", "CREATED");
                println!("{}", "-".repeat(80));
                for (key, label, created) in &keys {
                    // mask middle of key: kms_xxxx...xxxx
                    let masked = if key.len() > 12 {
                        format!("{}...{}", &key[..8], &key[key.len()-4..])
                    } else {
                        key.clone()
                    };
                    println!("{:<40} {:<20} {}", masked, label, created);
                }
                println!("\n{} key(s) total.", keys.len());
            }
        }
        "revoke" => {
            let key = args.get(2).expect("Usage: api-key revoke <KEY>");
            if db.revoke_api_key(key)? {
                println!("API key revoked.");
            } else {
                println!("API key not found.");
            }
        }
        _ => {
            eprintln!("KMS API Key Management");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  api-key generate [--label \"my-service\"]  Generate new API key");
            eprintln!("  api-key list                             List all API keys");
            eprintln!("  api-key revoke <KEY>                     Revoke an API key");
        }
    }
    Ok(())
}
