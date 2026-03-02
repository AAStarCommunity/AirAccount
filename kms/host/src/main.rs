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

use kms::{cli, tests, create_wallet, derive_address, sign_transaction};

use anyhow::{bail, Result};
use structopt::StructOpt;

fn main() -> Result<()> {
    let args = cli::Opt::from_args();
    match args.command {
        cli::Command::CreateWallet(_opt) => {
            let wallet_id = create_wallet()?;
            println!("Wallet ID: {}", wallet_id);
        }
        cli::Command::DeriveAddress(opt) => {
            let address = derive_address(opt.wallet_id, &opt.hd_path)?;
            println!("Address: 0x{}", hex::encode(&address));
        }
        cli::Command::SignTransaction(opt) => {
            let signature = sign_transaction(
                opt.wallet_id,
                &opt.hd_path,
                opt.chain_id,
                opt.nonce,
                opt.to,
                opt.value,
                opt.gas_price,
                opt.gas,
            )?;
            println!("Signature: {}", hex::encode(&signature));
        }
        cli::Command::Test => {
            tests::tests::test_workflow();
            println!("Tests passed");
        }
        _ => {
            bail!("Unsupported command");
        }
    }
    Ok(())
}
