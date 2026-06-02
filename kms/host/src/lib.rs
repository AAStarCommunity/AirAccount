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

//! KMS Host Library
//! Shared modules for CLI and API server

pub mod address_cache;
pub mod agent_jwt;
pub mod cli;
pub mod db;
pub mod rate_limit;
#[cfg(feature = "tee")]
pub mod ta_client;
#[cfg(feature = "tee")]
pub mod tests;
pub mod webauthn;

// Re-export commonly used items
pub use address_cache::{
    load_address_map, lookup_address, save_address_map, update_address_entry, AddressMap,
    AddressMetadata,
};
#[cfg(feature = "tee")]
pub use ta_client::{create_wallet, derive_address, sign_transaction, TaClient, TeeHandle};
