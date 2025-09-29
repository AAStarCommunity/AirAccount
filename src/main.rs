mod api;
mod eth_wallet;

use api::start_kms_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 设置日志
    env_logger::init();

    println!("🚀 启动基于 eth_wallet TA 的 KMS API 服务器");

    // 启动 KMS API 服务器
    start_kms_server().await?;

    Ok(())
}