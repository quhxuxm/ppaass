use std::net::SocketAddr;

use log::{debug, error, info};
use tokio::net::{TcpListener, TcpStream};

use crate::{config::AGENT_CONFIG, error::NetworkError};
use crate::{error::AgentError, processor::dispatcher::ClientProtocolDispatcher};

#[derive(Debug, Default)]
pub struct AgentServer {}

impl AgentServer {
    async fn accept_client_connection(tcp_listener: &TcpListener) -> Result<(TcpStream, SocketAddr), AgentError> {
        let (client_tcp_stream, client_socket_address) = tcp_listener.accept().await.map_err(NetworkError::Io)?;
        client_tcp_stream.set_nodelay(true).map_err(NetworkError::Io)?;
        Ok((client_tcp_stream, client_socket_address))
    }

    pub async fn start(&mut self) -> Result<(), AgentError> {
        let agent_server_bind_addr = if AGENT_CONFIG.get_ipv6() {
            format!("::1:{}", AGENT_CONFIG.get_port())
        } else {
            format!("0.0.0.0:{}", AGENT_CONFIG.get_port())
        };
        info!("Agent server start to serve request on address: {agent_server_bind_addr}.");

        let tcp_listener = TcpListener::bind(&agent_server_bind_addr).await.map_err(NetworkError::Io)?;
        // let proxy_connection_pool = Box::new(ProxyConnectionPool::new().await?);
        // let proxy_connection_pool: &'static ProxyConnectionPool = Box::leak(proxy_connection_pool);
        loop {
            let (client_tcp_stream, client_socket_address) = match Self::accept_client_connection(&tcp_listener).await {
                Ok(accept_result) => accept_result,
                Err(e) => {
                    error!("Agent server fail to accept client connection because of error: {e:?}");
                    continue;
                },
            };
            debug!("Accept client tcp connection on address: {}", client_socket_address);
            tokio::spawn(async move {
                let processor = match ClientProtocolDispatcher::dispatch(client_tcp_stream, client_socket_address).await {
                    Err(e) => {
                        error!(
                            "Client tcp connection [{client_socket_address}] fail to dispatch client tcp connection to concrete flow because of error: {e:?}"
                        );
                        return;
                    },
                    Ok(v) => v,
                };
                if let Err(e) = processor.exec().await {
                    error!("Client tcp connection [{client_socket_address}] fail to execute client flow because of error: {e:?}");
                    return;
                };
                debug!("Client tcp connection [{client_socket_address}] complete to serve.")
            });
        }
    }
}
