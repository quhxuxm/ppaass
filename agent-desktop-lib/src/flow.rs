pub(crate) mod dispatcher;

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;

use deadpool::managed::Pool;

use tokio::io::{AsyncRead, AsyncWrite};

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
        mut self, proxy_connection_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>,
        rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        match self {
            ClientFlow::Http { stream, client_socket_address } => {
                todo!()
            },
            ClientFlow::Socks5 { stream, client_socket_address } => {
                let socks5_flow = Socks5Flow::new(stream, client_socket_address);
                socks5_flow.exec(proxy_connection_pool, configuration, rsa_crypto_fetcher).await?;
            },
        }
        Ok(())
    }
}
