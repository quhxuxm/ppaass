use bytes::{BufMut, BytesMut};

use futures::{try_join, SinkExt, StreamExt};
use ppaass_common::{
    tcp_loop::{TcpLoopInitResponsePayload, TcpLoopInitResponseType},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryption, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload,
    PpaassMessageProxyPayloadParts, PpaassMessageProxyPayloadType, PpaassNetAddress,
};

use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::timeout,
};
use tokio_util::codec::{Framed, FramedParts};
use tracing::{debug, error, trace};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    flow::socks::{
        codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
        message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent},
    },
    pool::{ProxyConnectionPool, ProxyConnectionRead, ProxyConnectionWrite},
    AgentServerPayloadEncryptionTypeSelector, SOCKS_V5,
};
use anyhow::{Context, Result};
use ppaass_common::generate_uuid;

mod codec;
mod message;

struct Socks5RelayInfo<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    client_io: T,
    client_socket_address: SocketAddr,
    tcp_loop_key: String,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption,
    proxy_connection_read: ProxyConnectionRead,
    proxy_connection_write: ProxyConnectionWrite,
    configuration: Arc<AgentServerConfig>,
}

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

    async fn relay(info: Socks5RelayInfo<T>) -> Result<()> {
        let client_io = info.client_io;
        let tcp_loop_key = info.tcp_loop_key;
        let client_socket_address = info.client_socket_address;
        let configuration = info.configuration;
        let payload_encryption = info.payload_encryption;
        let mut proxy_connection_write = info.proxy_connection_write;
        let user_token = info.user_token;
        let mut proxy_connection_read = info.proxy_connection_read;
        let (mut client_io_read, mut client_io_write) = tokio::io::split(client_io);
        let tcp_loop_key_a2p = tcp_loop_key.clone();

        let mut client_to_proxy_relay_guard = tokio::spawn(async move {
            debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] start to relay from agent to proxy.");
            loop {
                let mut client_message = Vec::with_capacity(configuration.get_client_io_buffer_size());
                match timeout(
                    Duration::from_secs(configuration.get_client_read_timeout()),
                    client_io_read.read_buf(&mut client_message),
                )
                .await
                {
                    Err(_) => {
                        error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to read from client because of timeout");
                        return Err(anyhow::anyhow!(format!(
                            "Client tcp connection [{}] for tcp loop [{}] fail to read from client because of timeout",
                            client_socket_address, tcp_loop_key_a2p
                        )));
                    },
                    Ok(Ok(0)) => {
                        debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] complete to relay from client to proxy.");
                        break;
                    },
                    Ok(Ok(client_message)) => client_message,
                    Ok(Err(e)) => {
                        error!(
                            "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to read from client because of error: {e:?}"
                        );
                        return Err(anyhow::anyhow!(e));
                    },
                };

                trace!(
                    "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] read client data:\n{}\n",
                    pretty_hex::pretty_hex(&client_message)
                );
                let agent_message = PpaassMessageGenerator::generate_raw_data(&user_token, payload_encryption.clone(), client_message.to_vec())?;
                if let Err(e) = proxy_connection_write.send(agent_message).await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to relay client data to proxy because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }
            Ok::<_, anyhow::Error>(())
        });

        let tcp_loop_key_p2a = tcp_loop_key.clone();

        let mut proxy_to_client_relay_guard = tokio::spawn(async move {
            debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] start to relay from proxy to agent.");
            while let Some(proxy_message) = proxy_connection_read.next().await {
                if let Err(e) = proxy_message {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to read from proxy because of error: {e:?}");
                    if let Err(e) = client_io_write.shutdown().await {
                        error!(
                            "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to shutdown client io because of error: {e:?}"
                        );
                    }
                    return Err(e);
                }
                let proxy_message = proxy_message.expect("Should not panic when read proxy message");
                let PpaassMessageParts {
                    payload_bytes: proxy_message_payload_bytes,
                    ..
                } = proxy_message.split();
                trace!(
                    "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] read proxy data:\n{}\n",
                    pretty_hex::pretty_hex(&proxy_message_payload_bytes)
                );
                if let Err(e) = client_io_write.write_all(&proxy_message_payload_bytes).await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to relay to proxy because of error: {e:?}");
                    if let Err(e) = client_io_write.shutdown().await {
                        error!(
                            "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to shutdown client io because of error: {e:?}"
                        );
                    }
                    return Err(anyhow::anyhow!(e));
                };
                if let Err(e) = client_io_write.flush().await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to relay to proxy because of error(flush): {e:?}");
                    if let Err(e) = client_io_write.shutdown().await {
                        error!(
                            "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to shutdown client io because of error: {e:?}"
                        );
                    }
                    return Err(anyhow::anyhow!(e));
                };
            }
            debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] complete to relay from proxy to agent.");
            if let Err(e) = client_io_write.shutdown().await {
                error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to shutdown client io because of error: {e:?}");
                return Err(anyhow::anyhow!(e));
            };

            Ok::<_, anyhow::Error>(())
        });
        if let Err(e) = try_join!(&mut client_to_proxy_relay_guard, &mut proxy_to_client_relay_guard) {
            client_to_proxy_relay_guard.abort();
            proxy_to_client_relay_guard.abort();
            error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key}] fail to do relay process because of error: {e:?}",);
        };
        Ok(())
    }

    pub(crate) async fn exec(self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>) -> Result<()> {
        let client_io = self.stream;
        let client_socket_address = self.client_socket_address;
        let mut auth_framed_parts = FramedParts::new(client_io, Socks5AuthCommandContentCodec);
        let mut auth_initial_buf = BytesMut::new();
        auth_initial_buf.put_u8(SOCKS_V5);
        auth_framed_parts.read_buf = auth_initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = auth_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_socket_address}] nothing to read from socks5 client in authenticate phase"
            ))?
            .context(format!(
                "Client tcp connection [{client_socket_address}] error happen when read socks5 client data in authenticate phase"
            ))?;
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Client tcp connection [{client_socket_address}] start socks5 authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        if let Err(e) = auth_framed.send(auth_response).await {
            error!("Client tcp connection [{client_socket_address}] fail reply auth success in socks5 flow.");
            return Err(e);
        };

        let FramedParts { io: client_io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec);
        let init_message = init_framed
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_socket_address}] nothing to read from socks5 client in init phase"
            ))?
            .context(format!(
                "Client tcp connection [{client_socket_address}] error happen when read socks5 client data in init phase"
            ))?;
        let Socks5InitCommandContentParts { request_type, dest_address } = init_message.split();
        debug!(
            "Client tcp connection [{client_socket_address}] start socks5 init process, request type: {request_type:?}, destination address: {dest_address:?}"
        );
        let user_token = configuration
            .get_user_token()
            .as_ref()
            .context(format!(
                "Client tcp connection [{client_socket_address}] can not get user token form configuration file"
            ))?
            .clone();
        let src_address: PpaassNetAddress = self.client_socket_address.into();
        let dest_address: PpaassNetAddress = dest_address.into();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let proxy_connection = proxy_connection_pool.take_connection().await.context(format!(
            "Client tcp connection [{client_socket_address}] fail to take proxy connection from connection poool because of error"
        ))?;

        let tcp_loop_init_request =
            PpaassMessageGenerator::generate_tcp_loop_init_request(&user_token, src_address.clone(), dest_address.clone(), payload_encryption.clone())?;
        let proxy_connection_id = proxy_connection.id.clone();
        let (mut proxy_connection_read, mut proxy_connection_write) = proxy_connection.split()?;

        debug!("Client tcp connection [{client_socket_address}] take proxy connectopn [{proxy_connection_id}] to do proxy");

        if let Err(e) = proxy_connection_write.send(tcp_loop_init_request).await {
            error!("Client tcp connection [{client_socket_address}] fail to send tcp loop init to proxy because of error: {e:?}");
            return Err(anyhow::anyhow!(format!(
                "Client tcp connection [{client_socket_address}] fail to send tcp loop init request to proxy because of error: {e:?}"
            )));
        };

        let proxy_message = proxy_connection_read
            .next()
            .await
            .context(format!(
                "Client tcp connection [{client_socket_address}] nothing to read from proxy for init tcp loop response"
            ))?
            .context(format!(
                "Client tcp connection [{client_socket_address}] error happen when read proxy message for init tcp loop response"
            ))?;

        let PpaassMessageParts {
            payload_bytes: proxy_message_payload_bytes,
            user_token,
            ..
        } = proxy_message.split();
        let PpaassMessageProxyPayloadParts { payload_type, data } = TryInto::<PpaassMessageProxyPayload>::try_into(proxy_message_payload_bytes)?.split();
        let tcp_loop_init_response = match payload_type {
            PpaassMessageProxyPayloadType::TcpLoopInit => TryInto::<TcpLoopInitResponsePayload>::try_into(data)?,
            _ => {
                error!("Client tcp connection [{client_socket_address}] receive invalid message from proxy, payload type: {payload_type:?}");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{client_socket_address}] receive invalid message from proxy, payload type: {payload_type:?}"
                )));
            },
        };

        let TcpLoopInitResponsePayload {
            loop_key: tcp_loop_key,
            dest_address,
            response_type,
            ..
        } = tcp_loop_init_response;

        match response_type {
            TcpLoopInitResponseType::Success => {
                debug!("Client tcp connection [{client_socket_address}] receive init tcp loop init response: {tcp_loop_key}");
            },
            TcpLoopInitResponseType::Fail => {
                error!("Client tcp connection [{client_socket_address}] fail to do tcp loop init, tcp loop key: [{tcp_loop_key}]");
                return Err(anyhow::anyhow!(format!(
                    "Client tcp connection [{client_socket_address}] fail to do tcp loop init, tcp loop key: [{tcp_loop_key}]"
                )));
            },
        }

        let socks5_init_success_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dest_address.clone().into()));

        if let Err(e) = init_framed.send(socks5_init_success_result).await {
            error!("Client tcp connection [{client_socket_address}] fail reply init success in socks5 flow, tcp loop key: [{tcp_loop_key}].");
            return Err(e);
        };

        let FramedParts { io: client_io, .. } = init_framed.into_parts();
        debug!("Client tcp connection [{client_socket_address}] success to do sock5 handshake begin to relay, tcp loop key: [{tcp_loop_key}].");

        Self::relay(Socks5RelayInfo {
            client_io,
            client_socket_address,
            tcp_loop_key: tcp_loop_key.clone(),
            user_token,
            payload_encryption,
            proxy_connection_read,
            proxy_connection_write,
            configuration,
        })
        .await?;
        debug!("Client tcp connection [{client_socket_address}] complete sock5 relay, tcp loop key: [{tcp_loop_key}].");
        Ok(())
    }
}
