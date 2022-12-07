use std::{sync::Arc, time::Duration};

use crate::{config::ProxyServerConfig, connection::AgentConnection, crypto::ProxyServerRsaCryptoFetcher};

use anyhow::{Context, Result};
use tokio::{net::TcpListener, sync::Semaphore};
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

        let agent_connection_accept_timeout = self.configuration.get_agent_connection_accept_timeout();
        loop {
            let Ok((agent_tcp_stream, agent_socket_address)) =tcp_listener.accept().await else{
                error!("Fail to accept agent tcp connection.");
                continue;
            };

            let max_agent_connection_number_semaphore = self.max_agent_connection_number_semaphore.clone();

            agent_tcp_stream.set_nodelay(true).context("Fail to set no delay on agent tcp connection")?;
            debug!("Accept agent tcp connection on address: {}", agent_socket_address);
            let proxy_server_rsa_crypto_fetcher = rsa_crypto_fetcher.clone();
            let configuration = self.configuration.clone();
            tokio::spawn(async move {
                let _agent_connection_number_guard = match tokio::time::timeout(
                    Duration::from_secs(agent_connection_accept_timeout),
                    max_agent_connection_number_semaphore.acquire(),
                )
                .await
                {
                    Err(_) => {
                        error!("Accept agent connection timeout, drop the agent connection.");
                        return;
                    },
                    Ok(Ok(v)) => v,
                    Ok(Err(e)) => {
                        error!("Proxy server dropped, will not accept on new agent connection on this server instance: {e:?}");
                        return;
                    },
                };
                let agent_connection = AgentConnection::new(agent_tcp_stream, agent_socket_address.into(), configuration, proxy_server_rsa_crypto_fetcher);
                if let Err(e) = agent_connection.exec().await {
                    error!("Fail to execute agent connection because of error: {e:?}");
                };
            });
        }
    }
}
