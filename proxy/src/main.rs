mod common;
mod config;
mod crypto;
mod error;
mod processor;
mod server;

use config::PROXY_CONFIG;
use log::error;
use log::info;

use anyhow::Result;
use tokio::runtime::Builder;

use crate::server::ProxyServer;

fn main() -> Result<()> {
    log4rs::init_file("resources/config/ppaass-proxy-log.yaml", Default::default())?;

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
