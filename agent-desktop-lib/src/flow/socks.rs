use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use deadpool::managed::Pool;
use futures::{SinkExt, StreamExt};
use ppaass_io::PpaassMessageFramed;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, FramedParts};
use tracing::debug;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    flow::socks::{
        codec::Socks5InitCommandContentCodec,
        message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts},
    },
    pool::ProxyMessageFramedManager,
};

use self::codec::Socks5AuthCommandContentCodec;
use super::ClientFlow;
use anyhow::Context;
use anyhow::Result;
use ppaass_protocol::MessageUtil;

mod codec;
mod message;

pub(crate) struct Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    stream: T,
    client_socket_address: SocketAddr,
}

impl<T> Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    pub(crate) fn new(stream: T, client_socket_address: SocketAddr) -> Self {
        Self { stream, client_socket_address }
    }
}

#[async_trait]
impl<T> ClientFlow for Socks5ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    async fn exec(
        &mut self, proxy_message_framed_pool: Pool<ProxyMessageFramedManager>, configuration: Arc<AgentServerConfig>,
        rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
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

        let mut proxy_message_framed = proxy_message_framed_pool
            .get()
            .await
            .map_err(|e| anyhow::anyhow!(e))
            .context("Fail to get proxy server connection")?;

        MessageUtil::create_agent_tcp_initialize_request("user1");

        proxy_message_framed.next().await;
        todo!()
    }
}
