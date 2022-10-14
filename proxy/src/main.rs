use anyhow::Result;
use manager::ProxyServerManager;
mod arguments;
mod config;
mod constant;
mod crypto;
mod tunnel;
mod manager;
mod server;

fn main() -> Result<()> {
    let proxy_server_manager = ProxyServerManager::new()?;
    proxy_server_manager.start()?;
    Ok(())
}
