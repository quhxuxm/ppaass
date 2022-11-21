pub(crate) mod codec;

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use super::ClientFlow;
use crate::pool::ProxyConnectionManager;
use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};
use anyhow::Result;
use deadpool::managed::Pool;

pub(crate) struct HttpFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    stream: T,
    client_socket_address: SocketAddr,
}

impl<T> HttpFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    pub(crate) fn new(stream: T, client_socket_address: SocketAddr) -> Self {
        Self { stream, client_socket_address }
    }

    pub(crate) async fn exec(
        self, proxy_message_framed_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>,
        rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        todo!()
    }
}
