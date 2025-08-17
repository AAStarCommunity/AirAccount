use std::env;

fn main() {
    // TA 的 UUID - 使用相同的 UUID 以便测试
    let uuid = env::var("TA_UUID").unwrap_or_else(|_| "11223344-5566-7788-99aa-bbccddeeff01".to_string());
    println!("cargo:rustc-env=TA_UUID={}", uuid);
}