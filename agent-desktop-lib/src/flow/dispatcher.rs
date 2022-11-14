use std::sync::Arc;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, flow::socks::Socks5ClientFlow, pool::ProxyMessageFramedManager};
use anyhow::Result;

use deadpool::managed::Pool;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tracing::error;

use super::{http::HttpClientFlow, ClientFlow};

const SOCKS_V5: u8 = 5;
const SOCKS_V4: u8 = 4;

pub(crate) struct FlowDispatcher;

impl FlowDispatcher {
    pub(crate) async fn dispatch<T>(
        mut stream: T, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
        proxy_connection_pool: Pool<ProxyMessageFramedManager>,
    ) -> Result<Box<dyn ClientFlow>>
    where
        T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let protocol = stream.read_u8().await?;
        match protocol {
            SOCKS_V5 => {
                // For socks5 protocol
                Ok(Box::new(Socks5ClientFlow::new(
                    stream,
                    configuration,
                    rsa_crypto_fetcher,
                    proxy_connection_pool,
                )))
            },
            SOCKS_V4 => {
                // For socks4 protocol
                error!("Do not support socks v4 protocol");
                return Err(anyhow::anyhow!("do not support socks v4"));
            },
            _ => {
                // For http protocol
                Ok(Box::new(HttpClientFlow::new(stream, configuration, rsa_crypto_fetcher)))
            },
        }
    }
}
