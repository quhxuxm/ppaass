use std::net::SocketAddr;

use anyhow::Result;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tracing::error;

use super::ClientFlow;

const SOCKS_V5: u8 = 5;
const SOCKS_V4: u8 = 4;

pub(crate) struct FlowDispatcher;

impl FlowDispatcher {
    pub(crate) async fn dispatch<T>(mut stream: T, client_socket_address: SocketAddr) -> Result<ClientFlow<T>>
    where
        T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let protocol = stream.read_u8().await?;
        match protocol {
            SOCKS_V5 => {
                // For socks5 protocol
                Ok(ClientFlow::Socks5 { stream, client_socket_address })
            },
            SOCKS_V4 => {
                // For socks4 protocol
                error!("Do not support socks v4 protocol");
                return Err(anyhow::anyhow!("do not support socks v4"));
            },
            _ => {
                // For http protocol
                Ok(ClientFlow::Http { stream, client_socket_address })
            },
        }
    }
}
