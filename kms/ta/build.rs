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

use optee_utee_build::{Error, RustEdition, TaConfig};

fn main() -> Result<(), Error> {
    // p256-m: compile with -O1 -fPIC
    let mut cc_build = cc::Build::new();
    cc_build
        .file("p256-m.c")
        .opt_level(1)
        .pic(true)
        .flag("-fno-common")
        .warnings(false);
    // -marm is ARM32-only (force ARM mode, not Thumb); skip on aarch64
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("arm") || target.contains("arm-unknown") {
        cc_build.flag("-marm");
    }
    cc_build.compile("p256m");

    let ta_config = TaConfig::new_default_with_cargo_env(proto::UUID)?
        .ta_data_size(1024 * 1024)
        .ta_stack_size(128 * 1024);
    optee_utee_build::build(RustEdition::Before2024, ta_config)
}
