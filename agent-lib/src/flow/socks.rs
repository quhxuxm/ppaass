use bytes::BytesMut;

use futures::{SinkExt, StreamExt};
use ppaass_common::{
    tcp_loop::{TcpLoopInitResponsePayload, TcpLoopInitResponseType},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassMessageProxyPayload, PpaassMessageProxyPayloadParts,
    PpaassMessageProxyPayloadType, PpaassNetAddress,
};

use std::{net::SocketAddr, sync::Arc};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, FramedParts};
use tracing::{debug, error};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    flow::{
        socks::{
            codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
            message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent},
        },
        ClientDataRelayInfo, ClientFlow,
    },
    pool::ProxyConnectionPool,
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
    client_io: T,
    client_socket_address: SocketAddr,
}

impl<T> Socks5Flow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) fn new(client_io: T, client_socket_address: SocketAddr) -> Self {
        Self {
            client_io,
            client_socket_address,
        }
    }

    pub(crate) async fn exec(
        self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>, initial_buf: BytesMut,
    ) -> Result<()> {
        let client_io = self.client_io;
        let client_socket_address = self.client_socket_address;
        let mut auth_framed_parts = FramedParts::new(client_io, Socks5AuthCommandContentCodec::default());
        auth_framed_parts.read_buf = initial_buf;
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
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec::default());
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
        match request_type {
            message::Socks5InitCommandType::Connect => {
                let tcp_loop_init_request =
                    PpaassMessageGenerator::generate_tcp_loop_init_request(&user_token, src_address.clone(), dest_address.clone(), payload_encryption.clone())?;
                let proxy_connection = proxy_connection_pool.take_connection().await.context(format!(
                    "Client tcp connection [{client_socket_address}] fail to take proxy connection from connection poool because of error"
                ))?;

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
                let PpaassMessageProxyPayloadParts { payload_type, data } =
                    TryInto::<PpaassMessageProxyPayload>::try_into(proxy_message_payload_bytes)?.split();
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

                let socks5_init_success_result =
                    Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dest_address.clone().into()));

                if let Err(e) = init_framed.send(socks5_init_success_result).await {
                    error!("Client tcp connection [{client_socket_address}] fail reply init success in socks5 flow, tcp loop key: [{tcp_loop_key}].");
                    return Err(e);
                };

                let FramedParts { io: client_io, .. } = init_framed.into_parts();
                debug!("Client tcp connection [{client_socket_address}] success to do sock5 handshake begin to relay, tcp loop key: [{tcp_loop_key}].");

                ClientFlow::relay(ClientDataRelayInfo {
                    client_io,
                    client_socket_address,
                    tcp_loop_key: tcp_loop_key.clone(),
                    user_token,
                    payload_encryption,
                    proxy_connection_read,
                    proxy_connection_write,
                    configuration,
                    init_data: None,
                })
                .await?;
                debug!("Client tcp connection [{client_socket_address}] complete sock5 relay, tcp loop key: [{tcp_loop_key}].");
            },
            message::Socks5InitCommandType::Bind => todo!(),
            message::Socks5InitCommandType::UdpAssociate => todo!(),
        }

        Ok(())
    }
}
