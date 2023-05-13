use std::{net::SocketAddr, sync::Arc};

use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::NetworkError, pool::ProxyConnectionPool};
use crate::{error::AgentError, processor::dispatcher::ClientProtocolDispatcher};

pub struct AgentServer {
    configuration: Arc<AgentServerConfig>,
}

impl AgentServer {
    pub fn new(configuration: Arc<AgentServerConfig>) -> Self {
        Self { configuration }
    }

    async fn accept_client_connection(tcp_listener: &TcpListener) -> Result<(TcpStream, SocketAddr), AgentError> {
        let (client_tcp_stream, client_socket_address) = tcp_listener.accept().await.map_err(NetworkError::Io)?;
        client_tcp_stream.set_nodelay(true).map_err(NetworkError::Io)?;
        Ok((client_tcp_stream, client_socket_address))
    }

    pub async fn start(&mut self) -> Result<(), AgentError> {
        let agent_server_bind_addr = if self.configuration.get_ipv6() {
            format!("::1:{}", self.configuration.get_port())
        } else {
            format!("0.0.0.0:{}", self.configuration.get_port())
        };
        let rsa_crypto_fetcher = Arc::new(AgentServerRsaCryptoFetcher::new(self.configuration.clone())?);

        info!("Agent server start to serve request on address: {agent_server_bind_addr}.");

        let tcp_listener = TcpListener::bind(&agent_server_bind_addr).await.map_err(NetworkError::Io)?;
        let proxy_connection_pool = Arc::new(ProxyConnectionPool::new(self.configuration.clone(), rsa_crypto_fetcher.clone()).await?);
        loop {
            let (client_tcp_stream, client_socket_address) = match Self::accept_client_connection(&tcp_listener).await {
                Ok(accept_result) => accept_result,
                Err(e) => {
                    error!("Agent server fail to accept client connection because of error: {e:?}");
                    continue;
                },
            };
            debug!("Accept client tcp connection on address: {}", client_socket_address);
            let configuration = self.configuration.clone();
            let proxy_connection_pool = proxy_connection_pool.clone();
            tokio::spawn(async move {
                let processor = match ClientProtocolDispatcher::dispatch(client_tcp_stream, client_socket_address, &configuration).await {
                    Err(e) => {
                        error!(
                            "Client tcp connection [{client_socket_address}] fail to dispatch client tcp connection to concrete flow because of error: {e:?}"
                        );
                        return;
                    },
                    Ok(v) => v,
                };
                if let Err(e) = processor.exec(proxy_connection_pool, configuration).await {
                    error!("Client tcp connection [{client_socket_address}] fail to execute client flow because of error: {e:?}");
                    return;
                };
                debug!("Client tcp connection [{client_socket_address}] complete to serve.")
            });
        }
    }
}
