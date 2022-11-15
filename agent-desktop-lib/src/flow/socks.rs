use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use deadpool::managed::Pool;
use futures::{SinkExt, StreamExt};
use ppaass_protocol::{MessageUtil, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};
use std::borrow::BorrowMut;
use std::{net::SocketAddr, sync::Arc};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, FramedParts};
use tracing::debug;

use super::ClientFlow;
use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    flow::socks::{
        codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
        message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts},
    },
    pool::ProxyMessageFramedManager,
    AgentServerPayloadEncryptionTypeSelector,
};
use anyhow::{Context, Result};
use ppaass_common::generate_uuid;

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
        &mut self, proxy_connection_pool: Pool<ProxyMessageFramedManager>, configuration: Arc<AgentServerConfig>,
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

        let mut proxy_message_framed = proxy_connection_pool
            .get()
            .await
            .map_err(|e| anyhow::anyhow!(e))
            .context("Fail to get proxy server connection")?;
        let (mut proxy_message_framed_write, mut proxy_message_framed_read) = proxy_message_framed.as_mut().split();

        let user_token = configuration
            .get_user_token()
            .as_ref()
            .context("Can not get user token form configuration file")?;
        let src_address: PpaassNetAddress = self.client_socket_address.into();
        let dest_address: PpaassNetAddress = dest_address.into();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(user_token, Some(generate_uuid().into_bytes()));
        let tcp_session_init_request = MessageUtil::create_agent_tcp_session_initialize_request(&user_token, src_address, dest_address, payload_encryption)?;

        proxy_message_framed_write.send(tcp_session_init_request).await?;

        let Some(proxy_message) = proxy_message_framed_read.next().await else {
            return Err(anyhow::anyhow!("Nothing to read from proxy for tcp session init."));
        };
        let PpaassMessageParts {
            id: proxy_message_id,
            payload_bytes: proxy_message_payload_bytes,
            user_token,
            ..
        } = proxy_message?.split();

        todo!()
    }
}
