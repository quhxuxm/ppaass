use std::sync::Arc;

use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use futures::{SinkExt, StreamExt};
use snafu::ResultExt;
use tokio::{io::{AsyncRead, AsyncWrite}, net::TcpStream};
use tokio_util::codec::{Framed, FramedParts};
use tracing::debug;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    error::Error,
    flow::socks::{
        codec::Socks5InitCommandContentCodec,
        message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts},
    },
};

use self::{codec::Socks5AuthCommandContentCodec, message::Socks5AuthCommandContent};

use super::ClientFlow;
use crate::error::IoError;

mod codec;
mod message;

pub(crate) struct Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    stream: T,
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}

impl<T> Socks5ClientFlow<T>
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
impl<T> ClientFlow for Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    async fn exec(&mut self) -> Result<(), Error> {
        let mut auth_framed_parts = FramedParts::new(&mut self.stream, Socks5AuthCommandContentCodec);
        let mut auth_initial_buf = BytesMut::new();
        auth_initial_buf.put_u8(5);
        auth_framed_parts.read_buf = auth_initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = match auth_framed.next().await {
            None => return Ok(()),
            Some(result) => result?,
        };
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Socks5 connection in authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        auth_framed.send(auth_response).await?;
        let FramedParts { io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(io, Socks5InitCommandContentCodec);
        let init_message = match init_framed.next().await {
            None => return Ok(()),
            Some(result) => result?,
        };
        let Socks5InitCommandContentParts { request_type, dest_address } = init_message.split();
        debug!("Socks5 connection in init process, request type: {request_type:?}, destination address: {dest_address:?}");

        deadpool::managed::Pool::<TcpStream>::builder(manager)
        todo!()
    }
}
