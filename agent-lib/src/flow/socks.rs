use bytes::{BufMut, BytesMut};

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
use tracing::{debug, error};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    flow::socks::{
        codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
        message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent},
    },
    pool::{ProxyConnectionPool, ProxyMessageFramedRead, ProxyMessageFramedWrite},
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
    stream: T,
    client_socket_address: SocketAddr,
}

impl<T> Socks5Flow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) fn new(stream: T, client_socket_address: SocketAddr) -> Self {
        Self { stream, client_socket_address }
    }

    async fn relay<U>(
        client_io: T, client_socket_address: SocketAddr, tcp_loop_key: impl AsRef<str>, user_token: U, src_address: PpaassNetAddress,
        dest_address: PpaassNetAddress, payload_encryption: PpaassMessagePayloadEncryption, mut proxy_message_framed_read: ProxyMessageFramedRead,
        mut proxy_message_framed_write: ProxyMessageFramedWrite,
    ) -> Result<()>
    where
        U: AsRef<str> + Send + Debug + Display + Clone + 'static,
    {
        let (mut client_io_read, mut client_io_write) = tokio::io::split(client_io);
        let tcp_loop_key_a2p = tcp_loop_key.as_ref().to_owned();

        let a2p_guard = tokio::spawn(async move {
            debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] start to relay from agent to proxy.");
            loop {
                let mut buf = Vec::new();
                let size = match client_io_read.read(&mut buf).await {
                    Ok(v) => v,
                    Err(e) => {
                        error!(
                            "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to read from client because of error: {e:?}"
                        );
                        return Err(anyhow::anyhow!(e));
                    },
                };
                if size == 0 {
                    debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] complete to read from client.");
                    break;
                }
                debug!(
                    "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] read client data:\n{}\n",
                    pretty_hex::pretty_hex(&buf)
                );
                let agent_message = PpaassMessageGenerator::generate_raw_data(user_token.as_ref(), payload_encryption.clone(), buf[..size].to_vec())?;
                if let Err(e) = proxy_message_framed_write.send(agent_message).await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to relay client data to proxy because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
                if let Err(e) = proxy_message_framed_write.flush().await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to relay client data to proxy(flush) because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }
            Ok::<_, anyhow::Error>(())
        });

        let tcp_loop_key_p2a = tcp_loop_key.as_ref().to_owned();

        let p2a_guard = tokio::spawn(async move {
            debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] start to relay from proxy to agent.");
            while let Some(proxy_message) = proxy_message_framed_read.next().await {
                if let Err(e) = proxy_message {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to read from proxy because of error: {e:?}");
                    return Err(e);
                }
                let proxy_message = proxy_message.expect("Should not panic when read proxy message");
                let PpaassMessageParts {
                    id,
                    user_token,
                    payload_encryption,
                    payload_bytes,
                } = proxy_message.split();
                debug!(
                    "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] read proxy data:\n{}\n",
                    pretty_hex::pretty_hex(&payload_bytes)
                );
                if let Err(e) = client_io_write.write_all(&payload_bytes).await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to relay to proxy because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
                if let Err(e) = client_io_write.flush().await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to relay to proxy(flush) because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }
            Ok::<_, anyhow::Error>(())
        });
        if let Err(e) = try_join!(a2p_guard, p2a_guard) {
            error!(
                "Client tcp connection [{client_socket_address}] for tcp loop [{}] fail to do relay process because of error: {e:?}",
                tcp_loop_key.as_ref()
            );
        };
        Ok(())
    }

    pub(crate) async fn exec(
        self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        let client_io = self.stream;
        let client_sockst_address = self.client_socket_address;
        let mut auth_framed_parts = FramedParts::new(client_io, Socks5AuthCommandContentCodec);
        let mut auth_initial_buf = BytesMut::new();
        auth_initial_buf.put_u8(5);
        auth_framed_parts.read_buf = auth_initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = auth_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_sockst_address}] nothing to read from socks5 client in authenticate phase"
            ))?
            .context(format!(
                "Client tcp connection [{client_sockst_address}] error happen when read socks5 client data in authenticate phase"
            ))?;
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Client tcp connection [{client_sockst_address}] start socks5 authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        if let Err(e) = auth_framed.send(auth_response).await {
            error!("Client tcp connection [{client_sockst_address}] fail reply auth success in socks5 flow.");
            return Err(e);
        };
        if let Err(e) = auth_framed.flush().await {
            error!("Client tcp connection [{client_sockst_address}] fail reply auth(flush) success in socks5 flow.");
            return Err(e);
        };
        let FramedParts { io: client_io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec);
        let init_message = init_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_sockst_address}] nothing to read from socks5 client in init phase"
            ))?
            .context(format!(
                "Client tcp connection [{client_sockst_address}] error happen when read socks5 client data in init phase"
            ))?;
        let Socks5InitCommandContentParts { request_type, dest_address } = init_message.split();
        debug!(
            "Client tcp connection [{client_sockst_address}] start socks5 init process, request type: {request_type:?}, destination address: {dest_address:?}"
        );
        let user_token = configuration
            .get_user_token()
            .as_ref()
            .context(format!(
                "Client tcp connection [{client_sockst_address}] can not get user token form configuration file"
            ))?
            .clone();
        let src_address: PpaassNetAddress = self.client_socket_address.into();
        let dest_address: PpaassNetAddress = dest_address.into();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let proxy_connection = proxy_connection_pool.take_connection().await.context(format!(
            "Client tcp connection [{client_sockst_address}] fail to take proxy connection from connection poool because of error"
        ))?;

        let tcp_loop_init_request =
            PpaassMessageGenerator::generate_tcp_loop_init_request(&user_token, src_address.clone(), dest_address.clone(), payload_encryption.clone())?;

        let (mut proxy_connection_read, mut proxy_connection_write) = proxy_connection.split();

        if let Err(e) = proxy_connection_write.send(tcp_loop_init_request).await {
            error!("Client tcp connection [{client_sockst_address}] fail to send tcp loop init to proxy because of error: {e:?}");
            return Err(anyhow::anyhow!(format!(
                "Client tcp connection [{client_sockst_address}] receive invalid message from proxy, payload type: {payload_type:?}"
            )));
        };
        let proxy_message = proxy_connection_read
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_sockst_address}] nothing to read from proxy for init tcp loop response"
            ))?
            .context(format!(
                "Client tcp connection [{client_sockst_address}] error happen when read proxy message for init tcp loop response"
            ))?;

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
                error!("Client tcp connection [{client_sockst_address}] receive invalid message from proxy, payload type: {payload_type:?}");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{client_sockst_address}] receive invalid message from proxy, payload type: {payload_type:?}"
                )));
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
                debug!("Client tcp connection [{client_sockst_address}] receive init tcp loop init response: {loop_key}");
            },
            TcpLoopInitResponseType::Fail => {
                error!("Client tcp connection [{client_sockst_address}] fail to do tcp loop init, tcp loop key: [{loop_key}]");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{client_sockst_address}] fail to do tcp loop init, tcp loop key: [{loop_key}]"
                )));
            },
        }

        let socks5_init_success_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dest_address.clone().into()));

        if let Err(e) = init_framed.send(socks5_init_success_result).await {
            error!("Client tcp connection [{client_sockst_address}] fail reply init success in socks5 flow, tcp loop key: [{loop_key}].");
            return Err(e);
        };
        if let Err(e) = init_framed.flush().await {
            error!("Client tcp connection [{client_sockst_address}] fail reply(flush) init success in socks5 flow, tcp loop key: [{loop_key}].");
            return Err(e);
        };
        let FramedParts { io: client_io, .. } = init_framed.into_parts();
        debug!("Client tcp connection [{client_sockst_address}] success to do sock5 handshake begin to relay, tcp loop key: [{loop_key}].");

        Self::relay(
            client_io,
            client_sockst_address,
            loop_key,
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
