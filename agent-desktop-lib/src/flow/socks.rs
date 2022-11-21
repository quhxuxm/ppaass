use bytes::{BufMut, BytesMut};
use deadpool::managed::{Object, Pool};
use futures::{SinkExt, StreamExt};
use ppaass_protocol::{
    tcp_session_init::TcpSessionInitResponsePayload, tcp_session_relay::TcpSessionRelayPayload, PpaassMessageParts, PpaassMessagePayload,
    PpaassMessagePayloadEncryption, PpaassMessagePayloadEncryptionSelector, PpaassMessagePayloadParts, PpaassMessagePayloadType,
    PpaassMessageProxyPayloadTypeValue, PpaassMessageUtil, PpaassNetAddress,
};

use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    join,
};
use tokio_util::codec::{BytesCodec, Framed, FramedParts};
use tracing::{debug, error, info};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    flow::socks::{
        codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
        message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent},
    },
    pool::ProxyConnectionManager,
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

    async fn relay<U, S>(
        client_io: T, user_token: U, session_key: S, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, proxy_connection: Object<ProxyConnectionManager>,
    ) -> Result<()>
    where
        U: AsRef<str> + Send + 'static,
        S: AsRef<str> + Send + 'static,
    {
        let client_relay_framed = Framed::with_capacity(client_io, BytesCodec::new(), 1024 * 64);
        let (mut client_relay_framed_write, mut client_relay_framed_read) = client_relay_framed.split::<BytesMut>();
        let proxy_connection_id = proxy_connection.get_id().clone();
        let proxy_connection_read = proxy_connection.get_reader();
        let proxy_connection_write = proxy_connection.get_writer();
        let agnet_to_proxy_relay_guard = tokio::spawn(async move {
            while let Some(client_data) = client_relay_framed_read.next().await {
                match client_data {
                    Err(e) => {
                        error!("Fail to read client data because of error: {e:?}");
                        return Err(anyhow::anyhow!(e));
                    },
                    Ok(client_data) => {
                        info!(
                            "Read client data going to send to proxy connection [{:?}]: \n{}\n",
                            proxy_connection_id,
                            pretty_hex::pretty_hex(&client_data)
                        );
                        let agent_message = PpaassMessageUtil::create_tcp_session_relay(
                            user_token.as_ref(),
                            session_key.as_ref(),
                            src_address.clone(),
                            dest_address.clone(),
                            payload_encryption.clone(),
                            client_data.to_vec(),
                            true,
                        )?;
                        let mut proxy_connection_write = proxy_connection_write.lock().await;
                        if let Err(e) = proxy_connection_write.send(agent_message).await {
                            proxy_connection_write.close().await?;
                            return Err(anyhow::anyhow!(e));
                        };
                    },
                }
            }
            Ok::<_, anyhow::Error>(())
        });
        let proxy_to_agnet_relay_guard = tokio::spawn(async move {
            let mut proxy_connection_read = proxy_connection_read.lock().await;
            while let Some(proxy_data) = proxy_connection_read.next().await {
                match proxy_data {
                    Err(e) => {
                        error!("Fail to read proxy data because of error: {e:?}");
                        return Err(anyhow::anyhow!(e));
                    },
                    Ok(proxy_data) => {
                        let PpaassMessageParts { payload_bytes, .. } = proxy_data.split();
                        let PpaassMessagePayloadParts { payload_type, data } = TryInto::<PpaassMessagePayload>::try_into(payload_bytes)?.split();
                        match payload_type {
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionRelay) => {
                                let tcp_session_relay: TcpSessionRelayPayload = data.try_into()?;
                                client_relay_framed_write.send(BytesMut::from_iter(tcp_session_relay.data)).await?;
                            },
                            payload_type => {
                                error!("Fail to read proxy data because of invalid payload type: {payload_type:?}");
                                return Err(anyhow::anyhow!(format!(
                                    "Fail to read proxy data because of invalid payload type: {payload_type:?}"
                                )));
                            },
                        }
                    },
                }
            }
            Ok::<_, anyhow::Error>(())
        });
        let _ = join!(agnet_to_proxy_relay_guard, proxy_to_agnet_relay_guard);

        Ok(())
    }

    pub(crate) async fn exec(
        self, proxy_connection_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()> {
        let client_io = self.stream;
        let mut auth_framed_parts = FramedParts::new(client_io, Socks5AuthCommandContentCodec);
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
        let FramedParts { io: client_io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec);
        let init_message = match init_framed.next().await {
            None => return Ok(()),
            Some(result) => result?,
        };
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
        let mut proxy_connection = proxy_connection_pool
            .get()
            .await
            .map_err(|e| anyhow::anyhow!(e))
            .context("Fail to get proxy server connection")?;
        let (tcp_session_init_response, client_io) = {
            let proxy_connection_read = proxy_connection.get_reader();
            let proxy_connection_write = proxy_connection.get_writer();

            let tcp_session_init_request = PpaassMessageUtil::create_agent_tcp_session_initialize_request(
                &user_token,
                src_address.clone(),
                dest_address.clone(),
                payload_encryption.clone(),
            )?;

            {
                let mut proxy_connection_write = proxy_connection_write.lock().await;
                if let Err(e) = proxy_connection_write.send(tcp_session_init_request).await {
                    if let Err(e) = proxy_connection_write.close().await {
                        error!("Fail to close proxy connection because of error: {e:?}")
                    };
                    let _ = Object::take(proxy_connection);
                    return Err(anyhow::anyhow!(e));
                };
            }
            let proxy_message = {
                let mut proxy_connection_read = proxy_connection_read.lock().await;

                let Some(proxy_message) = proxy_connection_read.next().await else {
                    return Err(anyhow::anyhow!("Nothing to read from proxy for tcp session init."));
                };
                let Ok(proxy_message) = proxy_message else {
                    let mut proxy_connection_write = proxy_connection_write.lock().await;
                    if let Err(e) = proxy_connection_write.close().await {
                        error!("Fail to close proxy connection because of error: {e:?}")
                    };
                    let _ = Object::take(proxy_connection);
                    return Err(anyhow::anyhow!("Can not read porxy message for sock5 init."));
                };
                proxy_message
            };

            let PpaassMessageParts {
                id: proxy_message_id,
                payload_bytes: proxy_message_payload_bytes,
                user_token,
                ..
            } = proxy_message.split();
            let PpaassMessagePayloadParts { payload_type, data } = TryInto::<PpaassMessagePayload>::try_into(proxy_message_payload_bytes)?.split();
            let tcp_session_init_response = match payload_type {
                PpaassMessagePayloadType::AgentPayload(_) => {
                    let socks5_init_fail_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Failure, Some(dest_address.try_into()?));
                    init_framed.send(socks5_init_fail_result).await?;
                    return Err(anyhow::anyhow!("Invalid message payload type."));
                },
                PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeFail) => {
                    let socks5_init_fail_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Failure, Some(dest_address.try_into()?));
                    init_framed.send(socks5_init_fail_result).await?;
                    return Err(anyhow::anyhow!("Fail to initialize tcp session."));
                },
                PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeSuccess) => {
                    let tcp_session_init_response: TcpSessionInitResponsePayload = data.try_into()?;
                    tcp_session_init_response
                },
                PpaassMessagePayloadType::ProxyPayload(payload_type) => {
                    error!("Invalid message payload type: {payload_type:?}");
                    let socks5_init_fail_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Failure, Some(dest_address.try_into()?));
                    init_framed.send(socks5_init_fail_result).await?;
                    return Err(anyhow::anyhow!(format!("Invalid message payload type: {payload_type:?}")));
                },
            };
            debug!("Success init tcp session: {:?}", tcp_session_init_response.session_key);
            let socks5_init_success_result =
                Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dest_address.clone().try_into()?));
            init_framed.send(socks5_init_success_result).await?;
            let FramedParts { io: client_io, .. } = init_framed.into_parts();
            debug!("Begin to relay socks5 data for tcp session: {:?}", tcp_session_init_response.session_key);
            (tcp_session_init_response, client_io)
        };
        Self::relay(
            client_io,
            user_token,
            tcp_session_init_response.session_key.context("No session key assigend")?,
            src_address,
            dest_address,
            payload_encryption,
            proxy_connection,
        )
        .await?;
        Ok(())
    }
}
