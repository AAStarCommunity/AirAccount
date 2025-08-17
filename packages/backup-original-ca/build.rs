fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rustc-link-search=native=../../target/optee/optee_client/export_arm64/usr/lib");
    println!("cargo:rustc-link-lib=teec");
    Ok(())
}