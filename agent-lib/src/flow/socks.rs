use bytes::{BufMut, BytesMut};
use deadpool::managed::Pool;
use futures::{try_join, SinkExt, StreamExt};
use ppaass_common::{
    tcp_loop::{TcpLoopInitResponsePayload, TcpLoopInitResponseType},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryption, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload,
    PpaassMessageProxyPayloadParts, PpaassMessageProxyPayloadType, PpaassNetAddress,
};

use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
    sync::Arc,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::codec::{Framed, FramedParts};
use tracing::debug;

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    flow::socks::{
        codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
        message::{
            Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent,
            Socks5InitCommandResultContentParts,
        },
    },
    pool::{ProxyConnection, ProxyConnectionPool, ProxyMessageFramedRead, ProxyMessageFramedWrite},
    AgentServerPayloadEncryptionTypeSelector,
};
use anyhow::{Context, Result};
use ppaass_common::generate_uuid;

mod codec;
mod message;

pub(crate) struct Socks5Flow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    stream: Option<T>,
    client_socket_address: SocketAddr,
}

impl<T> Socks5Flow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) fn new(stream: T, client_socket_address: SocketAddr) -> Self {
        Self {
            stream: Some(stream),
            client_socket_address,
        }
    }

    async fn relay<U>(
        client_io: T, user_token: U, src_address: PpaassNetAddress, dest_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption,
        mut proxy_message_framed_read: ProxyMessageFramedRead, mut proxy_message_framed_write: ProxyMessageFramedWrite,
    ) -> Result<()>
    where
        U: AsRef<str> + Send + Debug + Display + Clone + 'static,
    {
        let (mut client_io_read, mut client_io_write) = tokio::io::split(client_io);

        let a2p_guard = tokio::spawn(async move {
            loop {
                let mut buf = Vec::with_capacity(1024 * 64);
                let size = client_io_read.read(&mut buf).await?;
                if size == 0 {
                    break;
                }
                let agent_message = PpaassMessageGenerator::generate_raw_data(user_token.as_ref(), payload_encryption.clone(), buf[..size].to_vec())?;

                proxy_message_framed_write.send(agent_message).await?;
            }
            Ok::<_, anyhow::Error>(())
        });

        let p2a_guard = tokio::spawn(async move {
            while let Some(proxy_message) = proxy_message_framed_read.next().await {
                if let Err(e) = proxy_message {
                    return Err(e);
                }
                let proxy_message = proxy_message.expect("Should not panic when read proxy message");
                let PpaassMessageParts {
                    id,
                    user_token,
                    payload_encryption,
                    payload_bytes,
                } = proxy_message.split();
                client_io_write.write_all(&payload_bytes).await?;
            }
            Ok::<_, anyhow::Error>(())
        });
        try_join!(a2p_guard, p2a_guard)?;
        Ok(())
    }

    pub(crate) async fn exec(
        &mut self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        let client_io = self.stream.take().context("Fail to get client io stream")?;

        let mut auth_framed_parts = FramedParts::new(client_io, Socks5AuthCommandContentCodec);
        let mut auth_initial_buf = BytesMut::new();
        auth_initial_buf.put_u8(5);
        auth_framed_parts.read_buf = auth_initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = auth_framed
            .next()
            .await
            .context("Nothing to read from socks5 client in authenticate phase")?
            .context("Error happen when read socks5 client data in authenticate phase")?;
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Socks5 connection in authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        auth_framed.send(auth_response).await?;
        let FramedParts { io: client_io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec);
        let init_message = init_framed
            .next()
            .await
            .context("Nothing to read from socks5 client in init phase")?
            .context("Error happen when read socks5 client data in init phase")?;
        let Socks5InitCommandContentParts { request_type, dest_address } = init_message.split();
        debug!("Socks5 connection in init process, request type: {request_type:?}, destination address: {dest_address:?}");
        let user_token = configuration
            .get_user_token()
            .as_ref()
            .context("Can not get user token form configuration file")?
            .clone();
        let src_address: PpaassNetAddress = self.client_socket_address.into();
        let dest_address: PpaassNetAddress = dest_address.into();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let proxy_connection = proxy_connection_pool
            .take_connection()
            .await
            .context("Fail to take proxy connection from connection poool because of error")?;

        let tcp_loop_init_request =
            PpaassMessageGenerator::generate_tcp_loop_init_request(&user_token, src_address.clone(), dest_address.clone(), payload_encryption.clone())?;

        let (mut proxy_connection_read, mut proxy_connection_write) = proxy_connection.split();

        proxy_connection_write.send(tcp_loop_init_request).await?;
        let proxy_message = proxy_connection_read
            .next()
            .await
            .context("Nothing to read from proxy for init tcp loop response")?
            .context("Error happen when read proxy message for init tcp loop response")?;

        let PpaassMessageParts {
            id: proxy_message_id,
            payload_bytes: proxy_message_payload_bytes,
            user_token,
            ..
        } = proxy_message.split();
        let PpaassMessageProxyPayloadParts { payload_type, data } = TryInto::<PpaassMessageProxyPayload>::try_into(proxy_message_payload_bytes)?.split();
        let tcp_loop_init_response = match payload_type {
            PpaassMessageProxyPayloadType::TcpLoopInit => TryInto::<TcpLoopInitResponsePayload>::try_into(data)?,
            _ => {
                return Err(anyhow::anyhow!("Receive invalid response from proxy for tcp loop init."));
            },
        };

        let TcpLoopInitResponsePayload {
            loop_key,
            src_address,
            dest_address,
            response_type,
        } = tcp_loop_init_response;

        match response_type {
            TcpLoopInitResponseType::Success => {
                debug!("Agent connection receive init tcp loop init response: {loop_key}");
            },
            TcpLoopInitResponseType::Fail => {
                return Err(anyhow::anyhow!("Fail to do tcp lopp init"));
            },
        }

        let socks5_init_success_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dest_address.clone().into()));
        init_framed.send(socks5_init_success_result).await?;

        let FramedParts { io: client_io, .. } = init_framed.into_parts();

        Self::relay(
            client_io,
            user_token,
            src_address,
            dest_address,
            payload_encryption,
            proxy_connection_read,
            proxy_connection_write,
        )
        .await?;
        Ok(())
    }
}
