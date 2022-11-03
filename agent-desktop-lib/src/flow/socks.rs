use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::Framed;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::Error};

use super::ClientFlow;

mod message;

pub(crate) struct Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite,
{
    stream: T,
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}

impl<T> Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite,
{
    pub(crate) fn new(stream: T, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Self {
        Self {
            stream,
            configuration,
            rsa_crypto_fetcher,
        }
    }
}

#[async_trait]
impl<T> ClientFlow for Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite,
{
    async fn exec(&mut self) -> Result<(), Error> {
        let auth_framed = Framed::new(inner, codec);
        todo!()
    }
}
