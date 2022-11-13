use std::sync::Arc;

use crate::{common::AgentMessageFramed, config::ProxyServerConfig, crypto::ProxyServerRsaCryptoFetcher, tunnel::TcpTunnel};

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing::{debug, error, info};

pub(crate) struct ProxyServer {
    configuration: Arc<ProxyServerConfig>,
}

impl ProxyServer {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Self {
        Self { configuration }
    }

    pub(crate) async fn start(&mut self) -> Result<()> {
        let port = self.configuration.get_port();
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!("::1:{port}")
        } else {
            format!("0.0.0.0:{port}")
        };
        let rsa_crypto_fetcher = Arc::new(ProxyServerRsaCryptoFetcher::new(self.configuration.clone())?);
        let agent_connection_buffer_size = self.configuration.get_agent_connection_buffer_size();
        info!("Proxy server start to serve request on address: {server_bind_addr}.");
        let tcp_listener = TcpListener::bind(&server_bind_addr)
            .await
            .context(format!("Fail to bind tcp listener for proxy server: {server_bind_addr}"))?;
        loop {
            let Ok((agent_tcp_stream, agent_socket_address)) =tcp_listener.accept().await else{
                error!("Fail to accept agent tcp connection.");
                continue;
            };
            agent_tcp_stream.set_nodelay(true).context("fail to set no delay on agent tcp connection")?;
            debug!("Accept agent tcp connection on address: {}", agent_socket_address);
            let proxy_server_rsa_crypto_fetcher = rsa_crypto_fetcher.clone();
            let configuration = self.configuration.clone();
            let Ok(agent_message_framed) = AgentMessageFramed::new(
                agent_tcp_stream,
                self.configuration.get_compress(),
                agent_connection_buffer_size,
                proxy_server_rsa_crypto_fetcher.clone(),
            ) else{
                error!("Fail to generate agent message framed.");
                continue;
            };
            let tpc_tunnel = TcpTunnel::new(agent_message_framed, agent_socket_address, configuration);
            if let Err(e) = tpc_tunnel.exec().await {
                error!("Fail to execute tunnel because of error: {e:?}");
            };
        }
    }
}
