// Licensed to AirAccount under the Apache License, Version 2.0

use std::env;

fn main() {
    let uuid = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "airaccount-ta".to_string());
    
    optee_utee_build::build();
    
    println!("cargo:rerun-if-changed=build.rs");
}