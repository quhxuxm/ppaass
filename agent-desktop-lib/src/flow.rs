pub(crate) mod dispatcher;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;

use deadpool::managed::{Object, Pool};

use futures::SinkExt;
use socks::Socks5FlowStatus;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tracing::error;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    pool::{PooledProxyConnection, PooledProxyConnectionError, ProxyConnectionManager, ProxyConnectionPool},
};

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
        self, proxy_connection_pool: ProxyConnectionPool, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        match self {
            ClientFlow::Http { stream, client_socket_address } => {
                todo!()
            },
            ClientFlow::Socks5 { stream, client_socket_address } => {
                let mut socks5_flow = Socks5Flow::new(stream, client_socket_address);
                if let Err(PooledProxyConnectionError {
                    pooled_proxy_connection,
                    source,
                }) = socks5_flow.exec(proxy_connection_pool, configuration, rsa_crypto_fetcher).await
                {
                    error!("Error happen on socks5 flow for proxy connection[{pooled_proxy_connection:?}]: {source:?}");
                    if let Some(proxy_connection) = pooled_proxy_connection {
                        let _ = Object::take(proxy_connection);
                    };
                    return Err(source);
                };
            },
        }
        Ok(())
    }
}
