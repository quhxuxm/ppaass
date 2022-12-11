use std::net::SocketAddr;

use anyhow::Result;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tracing::{debug, error};

use crate::{SOCKS_V4, SOCKS_V5};

use super::ClientFlow;

pub(crate) struct FlowDispatcher;

impl FlowDispatcher {
    pub(crate) async fn dispatch<T>(mut client_io: T, client_socket_address: SocketAddr) -> Result<ClientFlow<T>>
    where
        T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let protocol = client_io.read_u8().await?;
        match protocol {
            SOCKS_V5 => {
                // For socks5 protocol
                debug!("Client tcp connection [{client_socket_address}] begin to serve socks 5 protocol");
                Ok(ClientFlow::Socks5 {
                    client_io,
                    client_socket_address,
                })
            },
            SOCKS_V4 => {
                // For socks4 protocol
                error!("Client tcp connection [{client_socket_address}] do not support socks v4 protocol");
                Err(anyhow::anyhow!("do not support socks v4"))
            },
            _ => {
                // For http protocol
                debug!("Client tcp connection [{client_socket_address}] begin to serve http protocol");
                Ok(ClientFlow::Http {
                    client_io,
                    client_socket_address,
                })
            },
        }
    }
}
