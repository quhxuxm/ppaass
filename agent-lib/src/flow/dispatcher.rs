use std::{mem::size_of, net::SocketAddr};

use anyhow::{anyhow, Result};

use bytes::BytesMut;
use futures::StreamExt;
use tokio::net::TcpStream;
use tokio_util::codec::{Decoder, Framed, FramedParts};
use tracing::{debug, error};

use crate::{SOCKS_V4, SOCKS_V5};

use super::ClientFlow;

pub(crate) enum Protocol {
    /// The client side choose to use HTTP proxy
    Http,
    /// The client side choose to use Socks5 proxy
    Socks5,
    Socks4,
}

pub(crate) struct SwitchClientProtocolDecoder;

impl Decoder for SwitchClientProtocolDecoder {
    type Item = Protocol;

    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Use the first byte to decide what protocol the client side is using.
        if src.len() < size_of::<u8>() {
            return Ok(None);
        }
        let protocol_flag = src[0];
        match protocol_flag {
            SOCKS_V5 => Ok(Some(Protocol::Socks5)),
            SOCKS_V4 => Ok(Some(Protocol::Socks4)),
            _ => Ok(Some(Protocol::Http)),
        }
    }
}

pub(crate) struct FlowDispatcher;

impl FlowDispatcher {
    pub(crate) async fn dispatch(client_io: TcpStream, client_socket_address: SocketAddr) -> Result<ClientFlow> {
        let mut client_framed = Framed::with_capacity(client_io, SwitchClientProtocolDecoder, 1024 * 64);

        let protocol = match client_framed.next().await {
            Some(Ok(v)) => v,
            Some(Err(e)) => {
                error!("Fail to read protocol from client io because of error: {e:?}");
                return Err(anyhow!("Fail to read protocol from client io because of nothing to read."));
            },
            None => {
                error!("Fail to read protocol from client io because of nothing to read.");
                return Err(anyhow!("Fail to read protocol from client io because of nothing to read."));
            },
        };

        match protocol {
            Protocol::Socks5 => {
                // For socks5 protocol
                let FramedParts {
                    io: client_io,
                    read_buf: initial_buf,
                    ..
                } = client_framed.into_parts();
                debug!("Client tcp connection [{client_socket_address}] begin to serve socks 5 protocol");
                Ok(ClientFlow::Socks5 {
                    client_io,
                    client_socket_address,
                    initial_buf,
                })
            },
            Protocol::Socks4 => {
                // For socks4 protocol
                error!("Client tcp connection [{client_socket_address}] do not support socks v4 protocol");
                Err(anyhow!("Client tcp connection [{client_socket_address}] do not support socks v4 protocol"))
            },
            Protocol::Http => {
                // For http protocol
                let FramedParts {
                    io: client_io,
                    read_buf: initial_buf,
                    ..
                } = client_framed.into_parts();
                debug!("Client tcp connection [{client_socket_address}] begin to serve http protocol");
                Ok(ClientFlow::Http {
                    client_io,
                    client_socket_address,
                    initial_buf,
                })
            },
        }
    }
}
