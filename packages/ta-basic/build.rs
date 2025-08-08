// Build script for TA - following eth_wallet pattern

use std::env;

fn main() {
    let uuid = env::var("TA_UUID").unwrap_or_else(|_| {
        // Generate a default UUID for AirAccount basic TA
        "6e256cba-fc4d-4941-ad09-2ca1860342dd".to_string()
    });

    let out_dir = env::var("OUT_DIR").unwrap();
    
    optee_utee_build::generate_ta_header(&uuid, &out_dir).unwrap();
}