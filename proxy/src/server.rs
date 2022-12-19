use std::{sync::Arc, time::Duration};

use crate::{config::ProxyServerConfig, connection::AgentConnection, crypto::ProxyServerRsaCryptoFetcher};

use anyhow::{Context, Result};

use tokio::{io::AsyncWriteExt, net::TcpListener, sync::Semaphore, time::timeout};
use tracing::{debug, error, info};

pub(crate) struct ProxyServer {
    configuration: Arc<ProxyServerConfig>,
    max_agent_connection_number_semaphore: Arc<Semaphore>,
}

impl ProxyServer {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Self {
        let max_agent_connection_number_semaphore = Arc::new(Semaphore::new(configuration.get_max_agent_connection_number()));
        Self {
            configuration,
            max_agent_connection_number_semaphore,
        }
    }

    pub(crate) async fn start(&mut self) -> Result<()> {
        let port = self.configuration.get_port();
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!("::1:{port}")
        } else {
            format!("0.0.0.0:{port}")
        };
        let rsa_crypto_fetcher = Arc::new(ProxyServerRsaCryptoFetcher::new(self.configuration.clone())?);
        info!("Proxy server start to serve request on address: {server_bind_addr}.");
        let tcp_listener = TcpListener::bind(&server_bind_addr)
            .await
            .context(format!("Fail to bind tcp listener for proxy server: {server_bind_addr}"))?;

        loop {
            debug!(
                "Agent connection number semaphore remaining: {}",
                self.max_agent_connection_number_semaphore.available_permits()
            );
            let max_agent_connection_number_semaphore = self.max_agent_connection_number_semaphore.clone();
            let agent_connection_number_guard = match max_agent_connection_number_semaphore.acquire_owned().await {
                Err(e) => {
                    error!("Can not accept more agent connection because of max number exceed: {e:?}");
                    continue;
                },
                Ok(v) => v,
            };
            let (agent_tcp_stream, agent_socket_address) = match tcp_listener.accept().await {
                Err(e) => {
                    error!("Fail to accept agent tcp connection because of error: {e:?}");
                    continue;
                },
                Ok(v) => v,
            };

            if let Err(e) = agent_tcp_stream.set_nodelay(true) {
                error!("Fail to set no delay on agent tcp connection because of error: {e:?}");
                continue;
            };
            debug!("Accept agent tcp connection on address: {}", agent_socket_address);
            let proxy_server_rsa_crypto_fetcher = rsa_crypto_fetcher.clone();
            let configuration = self.configuration.clone();
            tokio::spawn(async move {
                let agent_connection = AgentConnection::new(agent_tcp_stream, agent_socket_address.into(), configuration, proxy_server_rsa_crypto_fetcher);
                if let Err(e) = agent_connection.exec().await {
                    error!("Fail to execute agent connection [{agent_socket_address}] because of error: {e:?}");
                    drop(agent_connection_number_guard);
                    return Err(anyhow::anyhow!(e));
                };
                drop(agent_connection_number_guard);
                debug!("Complete execute agent connection [{agent_socket_address}].");
                Ok(())
            });
        }
    }
}
