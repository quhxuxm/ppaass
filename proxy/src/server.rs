use std::sync::Arc;

use anyhow::Result;

use ppaass_io::PpaassTcpConnection;
use tokio::net::TcpListener;
use tracing::{debug, error, info};

use crate::{config::ProxyServerConfig, crypto::ProxyServerRsaCryptoFetcher, tunnel::ProxyTcpTunnel, tunnel::ProxyTcpTunnelRepository};

pub(crate) struct ProxyServer {
    configuration: Arc<ProxyServerConfig>,
    proxy_tcp_tunnel_repository: ProxyTcpTunnelRepository,
}

impl ProxyServer {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Self {
        let proxy_tcp_tunnel_repository = ProxyTcpTunnelRepository::new();
        Self {
            configuration,
            proxy_tcp_tunnel_repository,
        }
    }

    pub(crate) async fn start(&mut self) -> Result<()> {
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!("::1:{}", self.configuration.get_port())
        } else {
            format!("0.0.0.0:{}", self.configuration.get_port())
        };
        let proxy_server_rsa_crypto_fetcher = Arc::new(ProxyServerRsaCryptoFetcher::new(self.configuration.clone())?);
        let agent_connection_buffer_size = self.configuration.get_agent_connection_buffer_size();

        info!("Proxy server start to serve request on address: {server_bind_addr}.");
        let tcp_listener = match TcpListener::bind(&server_bind_addr).await {
            Err(e) => {
                error!("Fail to bind proxy server on address: [{server_bind_addr}] because of error: {e:?}");
                return Err(anyhow::anyhow!(e));
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
            let agent_tcp_connection =
                match PpaassTcpConnection::new(agent_tcp_stream, false, agent_connection_buffer_size, proxy_server_rsa_crypto_fetcher.clone()) {
                    Err(e) => {
                        error!("Fail to handle agent tcp connection because of error: {e:?}");
                        continue;
                    },
                    Ok(v) => v,
                };

            let mut proxy_tcp_tunnel = ProxyTcpTunnel::new(agent_tcp_connection);
            let proxy_tcp_tunnel_id = proxy_tcp_tunnel.get_tunnel_id().to_string();

            tokio::spawn(async {
                if let Err(e) = &proxy_tcp_tunnel.exec().await {
                    error!("Fail to execute proxy tcp tunnel because of error: {e:?}");
                }
            });
            self.proxy_tcp_tunnel_repository.insert(proxy_tcp_tunnel_id, proxy_tcp_tunnel);
        }
        Ok(())
    }
}
