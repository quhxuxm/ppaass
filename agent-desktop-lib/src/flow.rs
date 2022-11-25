pub(crate) mod dispatcher;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;

use deadpool::managed::{Object, Pool};

use futures::SinkExt;
use socks::Socks5FlowStatus;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tracing::error;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, pool::ProxyConnectionManager};

use self::socks::Socks5Flow;

mod http;
mod socks;

pub(crate) enum ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    Http { stream: T, client_socket_address: SocketAddr },
    Socks5 { stream: T, client_socket_address: SocketAddr },
}

impl<T> ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) async fn exec(
        self, proxy_connection_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        match self {
            ClientFlow::Http { stream, client_socket_address } => {
                todo!()
            },
            ClientFlow::Socks5 { stream, client_socket_address } => {
                let mut socks5_flow = Socks5Flow::new(stream, client_socket_address);
                if let Err(e) = socks5_flow.exec(proxy_connection_pool, configuration, rsa_crypto_fetcher).await {
                    error!("Error happen on socks5 flow in stage [{:?}]: {:?}", e.status, e.source);
                    if let Some(mut client_stream) = e.client_stream {
                        client_stream.shutdown().await?;
                    };
                    if let Some(proxy_connection) = e.proxy_connection {
                        let proxy_connection = Object::take(proxy_connection);
                        let proxy_connection_writer = proxy_connection.get_writer();
                        let mut proxy_connection_writer = proxy_connection_writer.lock().await;
                        proxy_connection_writer.close().await?;
                    };
                    return Err(e.source);
                };
            },
        }
        Ok(())
    }
}