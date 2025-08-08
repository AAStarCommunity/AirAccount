// Licensed to AirAccount under the Apache License, Version 2.0
// Multi-chain wallet support

use super::{WalletError, WalletResult};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: String,
    pub coin_type: u32,
    pub gas_price_multiplier: f64,
    pub confirmation_blocks: u32,
}

pub struct ChainAdapter {
    _configs: HashMap<u64, ChainConfig>,
}

impl ChainAdapter {
    pub fn new(configs: HashMap<u64, ChainConfig>) -> WalletResult<Self> {
        Ok(Self { _configs: configs })
    }
}

pub struct MultiChainWallet;