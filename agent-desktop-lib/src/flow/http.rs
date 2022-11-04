pub(crate) mod codec;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::Error};

use super::ClientFlow;

pub(crate) struct HttpClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    stream: T,
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}

impl<T> HttpClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
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
impl<T> ClientFlow for HttpClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    async fn exec(&mut self) -> Result<(), Error> {
        todo!();
    }
}
