use std::sync::Arc;

use crate::error::UsupportedProtocolError;
use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::Error, flow::socks::Socks5ClientFlow};
use bytes::BytesMut;
use ppaass_common::RsaCryptoFetcher;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tracing::error;

use super::{http::HttpClientFlow, ClientFlow};

const SOCKS_V5: u8 = 5;
const SOCKS_V4: u8 = 4;

pub(crate) struct FlowDispatcher;

impl FlowDispatcher {
    pub(crate) async fn dispatch<T>(
        mut stream: T, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<Box<dyn ClientFlow>, Error>
    where
        T: AsyncRead + AsyncWrite + Unpin + 'static,
    {
        let protocol = stream.read_u8().await?;
        match protocol {
            SOCKS_V5 => {
                // For socks5 protocol
                Ok(Box::new(Socks5ClientFlow::new(stream, configuration, rsa_crypto_fetcher)))
            },
            SOCKS_V4 => {
                // For socks4 protocol
                error!("Do not support socks v4 protocol");
                return UsupportedProtocolError { message: "socks v4" }.fail();
            },
            _ => {
                // For http protocol
                Ok(Box::new(HttpClientFlow::new(stream, configuration, rsa_crypto_fetcher)))
            },
        }
    }
}
