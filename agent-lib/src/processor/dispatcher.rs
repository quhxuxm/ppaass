use std::{mem::size_of, net::SocketAddr};

use anyhow::{anyhow, Result};

use bytes::BytesMut;
use futures::StreamExt;
use tokio::net::TcpStream;
use tokio_util::codec::{Decoder, Framed, FramedParts};
use tracing::{debug, error};

use crate::{SOCKS_V4, SOCKS_V5};

use super::ClientProcessor;

pub(crate) enum ClientProtocol {
    /// The client side choose to use HTTP proxy
    Http,
    /// The client side choose to use Socks5 proxy
    Socks5,
    Socks4,
}

pub(crate) struct SwitchClientProtocolDecoder;

impl Decoder for SwitchClientProtocolDecoder {
    type Item = ClientProtocol;

    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Use the first byte to decide what protocol the client side is using.
        if src.len() < size_of::<u8>() {
            return Ok(None);
        }
        let protocol_flag = src[0];
        match protocol_flag {
            SOCKS_V5 => Ok(Some(ClientProtocol::Socks5)),
            SOCKS_V4 => Ok(Some(ClientProtocol::Socks4)),
            _ => Ok(Some(ClientProtocol::Http)),
        }
    }
}

pub(crate) struct ClientConnectionProcessorDispatcher;

impl ClientConnectionProcessorDispatcher {
    pub(crate) async fn dispatch(client_tcp_stream: TcpStream, client_socket_address: SocketAddr) -> Result<ClientProcessor> {
        let mut client_message_framed = Framed::with_capacity(client_tcp_stream, SwitchClientProtocolDecoder, 1024 * 64);
        let client_protocol = match client_message_framed.next().await {
            Some(Ok(client_protocol)) => client_protocol,
            Some(Err(e)) => {
                error!("Fail to read protocol from client io because of error: {e:?}");
                return Err(anyhow!("Fail to read protocol from client io because of nothing to read."));
            },
            None => {
                error!("Fail to read protocol from client io because of nothing to read.");
                return Err(anyhow!("Fail to read protocol from client io because of nothing to read."));
            },
        };

        match client_protocol {
            ClientProtocol::Socks5 => {
                // For socks5 protocol
                let FramedParts {
                    io: client_tcp_stream,
                    read_buf: initial_buf,
                    ..
                } = client_message_framed.into_parts();
                debug!("Client tcp connection [{client_socket_address}] begin to serve socks 5 protocol");
                Ok(ClientProcessor::Socks5 {
                    client_tcp_stream,
                    src_address: client_socket_address.into(),
                    initial_buf,
                })
            },
            ClientProtocol::Socks4 => {
                // For socks4 protocol
                error!("Client tcp connection [{client_socket_address}] do not support socks v4 protocol");
                Err(anyhow!("Client tcp connection [{client_socket_address}] do not support socks v4 protocol"))
            },
            ClientProtocol::Http => {
                // For http protocol
                let FramedParts {
                    io: client_tcp_stream,
                    read_buf: initial_buf,
                    ..
                } = client_message_framed.into_parts();
                debug!("Client tcp connection [{client_socket_address}] begin to serve http protocol");
                Ok(ClientProcessor::Http {
                    client_tcp_stream,
                    src_address: client_socket_address.into(),
                    initial_buf,
                })
            },
        }
    }
}
