use crate::{
    config::PROXY_CONFIG,
    error::{NetworkError, ProxyError},
    processor::Transport,
};

use std::net::SocketAddr;

use log::{debug, error, info};

use tokio::net::{TcpListener, TcpStream};

/// The ppaass proxy server.
#[derive(Default)]
pub(crate) struct ProxyServer;

impl ProxyServer {
    async fn accept_agent_connection(tcp_listener: &TcpListener) -> Result<(TcpStream, SocketAddr), ProxyError> {
        let (agent_tcp_stream, agent_socket_address) = tcp_listener.accept().await.map_err(NetworkError::AgentAccept)?;
        agent_tcp_stream.set_linger(None).map_err(NetworkError::AgentAccept)?;
        agent_tcp_stream.set_nodelay(true).map_err(NetworkError::AgentAccept)?;
        Ok((agent_tcp_stream, agent_socket_address))
    }

    /// Start the proxy server instance.
    pub(crate) async fn start() -> Result<(), ProxyError> {
        let port = PROXY_CONFIG.get_port();
        let server_bind_addr = if PROXY_CONFIG.get_ipv6() {
            format!("[::]:{port}")
        } else {
            format!("0.0.0.0:{port}")
        };
        info!(
            "Proxy server start to serve request on address(ip v6={}): {server_bind_addr}.",
            PROXY_CONFIG.get_ipv6()
        );
        let tcp_listener = TcpListener::bind(&server_bind_addr).await.map_err(NetworkError::PortBinding)?;
        loop {
            let (agent_tcp_stream, agent_socket_address) = match Self::accept_agent_connection(&tcp_listener).await {
                Ok(accept_result) => accept_result,
                Err(e) => {
                    error!("Proxy server fail to accept agent connection because of error: {e:?}");
                    continue;
                },
            };
            debug!("Proxy server success accept agent connection on address: {}", agent_socket_address);
            tokio::spawn(async move {
                let transport = Transport::new(agent_tcp_stream, agent_socket_address.into());
                transport.exec().await?;
                debug!("Complete execute agent connection [{agent_socket_address}].");
                Ok::<_, ProxyError>(())
            });
        }
    }
}
