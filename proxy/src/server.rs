use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;

use ppaass_io::PpaassTcpConnection;
use tokio::{
    net::TcpListener,
    runtime::{Builder, Runtime},
    sync::Mutex,
};
use tracing::{debug, error, info};

use crate::{config::ProxyServerConfig, crypto::ProxyServerRsaCryptoFetcher, tunnel::ProxyTcpTunnel};
pub(crate) struct ProxyServer {
    configuration: Arc<ProxyServerConfig>,
    runtime: Runtime,
}

impl ProxyServer {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Result<Self> {
        let mut runtime_builder = Builder::new_multi_thread();
        runtime_builder.enable_all();
        runtime_builder.thread_name("ppaass-proxy-server-runtime");
        runtime_builder.worker_threads(configuration.get_thread_number());
        let runtime = runtime_builder.build()?;
        Ok(Self { runtime, configuration })
    }

    pub(crate) fn start(&self) -> Result<()> {
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!("::1:{}", self.configuration.get_port())
        } else {
            format!("0.0.0.0:{}", self.configuration.get_port())
        };
        let proxy_server_rsa_crypto_fetcher = Arc::new(ProxyServerRsaCryptoFetcher::new(self.configuration.clone())?);
        let agent_connection_buffer_size = self.configuration.get_agent_connection_buffer_size();

        self.runtime.spawn(async move {
            info!("Proxy server start to serve request on address: {server_bind_addr}.");
            let tcp_listener = match TcpListener::bind(&server_bind_addr).await {
                Err(e) => {
                    error!("Fail to bind proxy server on address: [{server_bind_addr}] because of error: {e:?}");
                    return;
                },
                Ok(v) => v,
            };
            loop {
                let (agent_tcp_stream, agent_socket_address) = match tcp_listener.accept().await {
                    Err(e) => {
                        error!("Fail to accept agent tcp connection because of error: {e:?}");
                        continue;
                    },
                    Ok(v) => v,
                };
                let proxy_server_rsa_crypto_fetcher = proxy_server_rsa_crypto_fetcher.clone();
                tokio::spawn(async move {
                    debug!("Begin to handle agent tcp connection, tcp stream: {agent_tcp_stream:?},socket address: {agent_socket_address:?}");
                    let agent_tcp_connection =
                        match PpaassTcpConnection::new(agent_tcp_stream, false, agent_connection_buffer_size, proxy_server_rsa_crypto_fetcher.clone()) {
                            Err(e) => {
                                error!("Fail to handle agent tcp connection because of error: {e:?}");
                                return;
                            },
                            Ok(v) => v,
                        };
                    let proxy_tcp_tunnel = ProxyTcpTunnel::new(agent_tcp_connection);
                    let proxy_tcp_tunnel_id = proxy_tcp_tunnel.get_id().to_string();
                    if let Err(e) = proxy_tcp_tunnel.exec().await {
                        error!("Fail to execute proxy tcp tunnel [{proxy_tcp_tunnel_id}] because of error: {e:?}");
                    }
                });
            }
        });
        Ok(())
    }

    pub(crate) fn shutdown(self) {
        info!("Going to shutdown proxy server in 3o seconds.");
        self.runtime.shutdown_timeout(Duration::from_secs(30));
    }
}
