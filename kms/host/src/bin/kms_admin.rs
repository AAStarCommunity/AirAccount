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

//! KMS Admin CLI — host-only admin operations (requires local machine access).
//!
//! Usage:
//!   kms-admin rotate-jwt-secret [--force]    # rotate kms_secret (emergency: --force invalidates all JWTs)
//!   kms-admin jwt-secret-status              # list kid versions, status, age
//!   kms-admin list-agent-keys [--account <wallet_id>]
//!   kms-admin revoke-agent-key <wallet_id>:<agent_index>

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

/// Parse compound agent keyId "wallet_uuid:agent_index"
fn parse_agent_key_id(key_id: &str) -> Result<(String, u32)> {
    let parts: Vec<&str> = key_id.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!(
            "Invalid agent keyId format (expected wallet_id:index): {}",
            key_id
        ));
    }
    let agent_index: u32 = parts[1]
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid agent_index: {}", parts[1]))?;
    Ok((parts[0].to_string(), agent_index))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match cmd {
        "rotate-jwt-secret" => cmd_rotate_jwt_secret(&args).await,
        "jwt-secret-status" => cmd_jwt_secret_status(),
        "list-agent-keys" => cmd_list_agent_keys(&args),
        "revoke-agent-key" => cmd_revoke_agent_key(&args),
        _ => {
            println!("KMS Admin CLI — host-access required");
            println!();
            println!("Usage:");
            println!("  kms-admin rotate-jwt-secret [--force]");
            println!(
                "    Rotate kms_secret (TEE). --force: immediately invalidates all active JWTs."
            );
            println!();
            println!("  kms-admin jwt-secret-status");
            println!("    List JWT key versions, status, age, retire_at.");
            println!();
            println!("  kms-admin list-agent-keys [--account <wallet_id>]");
            println!("    List all agent keys, optionally filtered by human wallet_id.");
            println!();
            println!("  kms-admin revoke-agent-key <wallet_id>:<agent_index>");
            println!("    Force-revoke an agent key (e.g. abc123:0).");
            Ok(())
        }
    }
}

async fn cmd_rotate_jwt_secret(args: &[String]) -> Result<()> {
    let force = args.iter().any(|a| a == "--force");

    if force {
        println!("WARNING: FORCE rotation — all active JWTs will be immediately invalidated.");
        println!("   Agents will need to refresh credentials via human WebAuthn.");
        println!();
    }

    #[cfg(feature = "tee")]
    {
        use kms::ta_client::TeeHandle;
        let tee = TeeHandle::new();
        let result = tee.jwt_rotate_secret(force).await?;

        let db = KmsDb::open(&db_path())?;
        let now = chrono::Utc::now().to_rfc3339();
        let retire_ts = chrono::Utc::now().timestamp() + 7 * 24 * 3600;

        db.upsert_jwt_secret_meta(&kms::db::JwtSecretMetaRow {
            kid: result.new_kid.clone(),
            status: "current".to_string(),
            created_at: now.clone(),
            retired_at: None,
            expires_at: None,
        })?;

        if let Some(ref old) = result.retired_kid {
            db.upsert_jwt_secret_meta(&kms::db::JwtSecretMetaRow {
                kid: old.clone(),
                status: if force {
                    "retired".to_string()
                } else {
                    "verify-only".to_string()
                },
                created_at: now,
                retired_at: None,
                expires_at: if force { None } else { Some(retire_ts) },
            })?;
        }

        println!("JWT secret rotated:");
        println!("   New kid:  {}", result.new_kid);
        if let Some(old) = result.retired_kid {
            if force {
                println!("   Old kid:  {} => retired (forced)", old);
            } else {
                println!("   Old kid:  {} => verify-only for 7 days", old);
            }
        }
    }

    #[cfg(not(feature = "tee"))]
    {
        eprintln!("rotate-jwt-secret requires TEE feature (run on KMS host with OP-TEE)");
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_jwt_secret_status() -> Result<()> {
    let db = KmsDb::open(&db_path())?;
    let metas = db.list_jwt_secret_meta()?;

    if metas.is_empty() {
        println!("No JWT secret metadata found. KMS may not have started yet.");
        return Ok(());
    }

    println!(
        "{:<8} {:<14} {:<28} {}",
        "KID", "STATUS", "CREATED_AT", "EXPIRES_AT"
    );
    println!("{}", "-".repeat(75));

    for row in &metas {
        let expires = row
            .expires_at
            .map(|t| t.to_string())
            .unwrap_or_else(|| "—".to_string());
        let status_icon = match row.status.as_str() {
            "current" => "current   ",
            "verify-only" => "verify-only",
            "retired" => "retired   ",
            s => s,
        };
        println!(
            "{:<8} {:<14} {:<28} {}",
            row.kid, status_icon, row.created_at, expires
        );
    }
    Ok(())
}

fn cmd_list_agent_keys(args: &[String]) -> Result<()> {
    let human_account = args
        .windows(2)
        .find(|w| w[0] == "--account")
        .map(|w| w[1].as_str());

    let db = KmsDb::open(&db_path())?;
    let keys = if let Some(acc) = human_account {
        db.list_agent_keys_for_human(acc)?
    } else {
        db.list_all_agent_keys()?
    };

    if keys.is_empty() {
        println!("No agent keys found.");
        return Ok(());
    }

    println!(
        "{:<40} {:<6} {:<42} {:<8} {}",
        "WALLET_ID", "IDX", "AGENT_ADDRESS", "STATUS", "CREATED_AT"
    );
    println!("{}", "-".repeat(140));
    for k in &keys {
        let key_id = format!("{}:{}", k.wallet_id, k.agent_index);
        println!(
            "{:<40} {:<6} {:<42} {:<8} {}",
            key_id, k.agent_index, k.agent_address, k.status, k.created_at
        );
    }
    println!("\n{} key(s) total.", keys.len());
    Ok(())
}

fn cmd_revoke_agent_key(args: &[String]) -> Result<()> {
    let key_id = args.get(2).ok_or_else(|| {
        anyhow::anyhow!("Usage: kms-admin revoke-agent-key <wallet_id>:<agent_index>")
    })?;

    let (wallet_id, agent_index) = parse_agent_key_id(key_id)?;

    let db = KmsDb::open(&db_path())?;
    let key = db
        .get_agent_key(&wallet_id, agent_index)?
        .ok_or_else(|| anyhow::anyhow!("Agent key not found: {}", key_id))?;

    if key.status == "revoked" {
        println!("Agent key {} is already revoked.", key_id);
        return Ok(());
    }

    let revoked = db.revoke_agent_key(&wallet_id, agent_index)?;
    if revoked {
        println!("Agent key {} revoked.", key_id);
        println!("   Address: {}", key.agent_address);
        println!("   Human account: {}", key.human_id);
    } else {
        println!("Failed to revoke agent key {}.", key_id);
    }
    Ok(())
}
