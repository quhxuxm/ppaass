use std::sync::Arc;

use crate::{config::ProxyServerConfig, crypto::ProxyServerRsaCryptoFetcher, processor::AgentConnectionProcessor};

use anyhow::{Context, Result};

use tokio::net::TcpListener;
use tracing::{debug, error, info};

/// The ppaass proxy server.
pub(crate) struct ProxyServer {
    configuration: Arc<ProxyServerConfig>,
}

impl ProxyServer {
    /// Create a new proxy server instance.
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Self {
        Self { configuration }
    }

    /// Start the proxy server instance.
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
                let agent_connection_processor =
                    AgentConnectionProcessor::new(agent_tcp_stream, agent_socket_address.into(), configuration, proxy_server_rsa_crypto_fetcher);
                if let Err(e) = agent_connection_processor.exec().await {
                    error!("Fail to execute agent connection [{agent_socket_address}] because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
                debug!("Complete execute agent connection [{agent_socket_address}].");
                Ok(())
            });
        }
    }
}
