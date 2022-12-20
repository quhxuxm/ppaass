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
        let mut protocol_buf = [0u8; 1];
        match client_io.read_exact(&mut protocol_buf).await {
            Ok(v) => v,
            Err(e) => {
                error!("Fail to read protocol from client io because of error: {e:?}");
                return Err(anyhow::anyhow!(e));
            },
        };
        match protocol_buf[0] {
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
                Err(anyhow::anyhow!(
                    "Client tcp connection [{client_socket_address}] do not support socks v4 protocol"
                ))
            },
            _ => {
                // For http protocol
                debug!("Client tcp connection [{client_socket_address}] begin to serve http protocol");
                Ok(ClientFlow::Http {
                    client_io,
                    client_socket_address,
                    protocol_data: protocol_buf[0],
                })
            },
        }
    }
}
