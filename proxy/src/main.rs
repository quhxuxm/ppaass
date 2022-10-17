use anyhow::Result;
use manager::ProxyServerManager;
mod arguments;
mod common;
mod config;
mod constant;
mod crypto;
mod transport;
mod manager;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    let proxy_server_manager = ProxyServerManager::new()?;
    proxy_server_manager.start().await?;
    Ok(())
}
