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

pub mod cli;
#[cfg(feature = "tee")]
pub mod ta_client;
#[cfg(feature = "tee")]
pub mod tests;
pub mod address_cache;
pub mod db;
pub mod rate_limit;
pub mod webauthn;

// Re-export commonly used items
#[cfg(feature = "tee")]
pub use ta_client::{TaClient, TeeHandle, create_wallet, derive_address, sign_transaction};
pub use address_cache::{AddressMetadata, AddressMap, load_address_map, save_address_map, update_address_entry, lookup_address};