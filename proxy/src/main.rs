mod common;
mod config;
mod crypto;
mod error;
mod processor;
mod server;

use config::PROXY_CONFIG;

use crate::server::ProxyServer;
use anyhow::Result;
use log::{error, info};
use tokio::runtime::Builder;

fn main() -> Result<()> {
    log4rs::init_file("resources/config/ppaass-proxy-log.yml", Default::default())?;
    let proxy_server_runtime = Builder::new_multi_thread()
        .enable_all()
        .thread_name("proxy-server-runtime")
        .worker_threads(PROXY_CONFIG.get_worker_thread_number())
        .build()?;

    proxy_server_runtime.block_on(async {
        info!("Begin to start proxy server.");
        let mut proxy_server = ProxyServer::default();
        if let Err(e) = proxy_server.start().await {
            error!("Fail to start proxy server because of error: {e:?}");
            panic!("Fail to start proxy server because of error: {e:?}")
        }
    });
    Ok(())
}
