use anyhow::Result;
use manager::ProxyServerManager;
mod arguments;
mod config;
mod constant;
mod manager;
mod crypto;
mod server;

fn main() -> Result<()> {
    let proxy_server_manager = ProxyServerManager::new()?;
    proxy_server_manager.start()?;
    Ok(())
}
