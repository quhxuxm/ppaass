use std::net::SocketAddr;
use std::sync::Arc;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, flow::socks::Socks5ClientFlow, pool::ProxyConnectionManager};
use anyhow::Result;

use deadpool::managed::Pool;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tracing::error;

use super::{http::HttpClientFlow, ClientFlow};

const SOCKS_V5: u8 = 5;
const SOCKS_V4: u8 = 4;

pub(crate) struct FlowDispatcher;

impl FlowDispatcher {
    pub(crate) async fn dispatch<T>(mut stream: T, client_socket_address: SocketAddr) -> Result<Box<dyn ClientFlow>>
    where
        T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let protocol = stream.read_u8().await?;
        match protocol {
            SOCKS_V5 => {
                // For socks5 protocol
                Ok(Box::new(Socks5ClientFlow::new(stream, client_socket_address)))
            },
            SOCKS_V4 => {
                // For socks4 protocol
                error!("Do not support socks v4 protocol");
                return Err(anyhow::anyhow!("do not support socks v4"));
            },
            _ => {
                // For http protocol
                Ok(Box::new(HttpClientFlow::new(stream, client_socket_address)))
            },
        }
    }
}
