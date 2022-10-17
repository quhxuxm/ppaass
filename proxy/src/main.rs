use anyhow::Result;
use manager::ProxyServerManager;
mod agent;
mod arguments;
mod config;
mod constant;
mod crypto;
mod manager;
mod server;
mod target;

#[tokio::main]
async fn main() -> Result<()> {
    let proxy_server_manager = ProxyServerManager::new()?;
    proxy_server_manager.start().await?;
    Ok(())
}
