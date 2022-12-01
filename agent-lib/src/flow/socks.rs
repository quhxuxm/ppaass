use bytes::{BufMut, BytesMut};
use deadpool::managed::Pool;
use futures::{try_join, SinkExt, StreamExt};
use ppaass_common::{
    tcp_loop::TcpSessionInitResponsePayload,
    tcp_session_relay::{TcpSessionRelayPayload, TcpSessionRelayStatus},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayload, PpaassMessagePayloadEncryption, PpaassMessagePayloadEncryptionSelector,
    PpaassMessagePayloadParts, PpaassMessagePayloadType, PpaassMessageProxyPayloadTypeValue, PpaassNetAddress,
};

use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
    sync::Arc,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{BytesCodec, Framed, FramedParts};
use tracing::{debug, error};

use self::message::Socks5InitCommandResultStatus;

use crate::{
    config::AgentServerConfig,
    crypto::AgentServerRsaCryptoFetcher,
    flow::socks::{
        codec::{Socks5AuthCommandContentCodec, Socks5InitCommandContentCodec},
        message::{Socks5AuthCommandContentParts, Socks5AuthCommandResultContent, Socks5InitCommandContentParts, Socks5InitCommandResultContent},
    },
    pool::{PooledProxyConnection, PooledProxyConnectionError, ProxyConnectionManager},
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

    async fn relay<U, S>(
        client_io: T, user_token: U, session_key: S, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, pooled_proxy_connection: PooledProxyConnection,
    ) -> Result<(), PooledProxyConnectionError>
    where
        U: AsRef<str> + Send + Debug + Display + Clone + 'static,
        S: AsRef<str> + Send + Debug + Display + Clone + PartialEq<String> + Eq + 'static,
    {
        let client_relay_framed = Framed::with_capacity(client_io, BytesCodec::new(), 1024 * 64);
        let (mut client_relay_framed_write, mut client_relay_framed_read) = client_relay_framed.split::<BytesMut>();
        let pooled_proxy_connection_id_a2p = pooled_proxy_connection.get_id().clone();
        let pooled_proxy_connection_id_p2a = pooled_proxy_connection.get_id().clone();
        let proxy_connection_read = pooled_proxy_connection.clone_reader();
        let proxy_connection_write = pooled_proxy_connection.clone_writer();
        let session_key_a2p = session_key.clone();
        let session_key_p2a = session_key.clone();

        let a2p_guard = tokio::spawn(async move {
            while let Some(client_data) = client_relay_framed_read.next().await {
                let client_data = client_data?;
                debug!(
                    "Tcp session [{session_key_a2p}] read client data, going to send to proxy connection [{:?}]: \n{}\n",
                    pooled_proxy_connection_id_a2p,
                    pretty_hex::pretty_hex(&client_data)
                );
                let agent_message = PpaassMessageGenerator::create_tcp_session_relay_data(
                    user_token.as_ref(),
                    session_key_a2p.as_ref(),
                    src_address.clone(),
                    dest_address.clone(),
                    payload_encryption.clone(),
                    client_data.to_vec(),
                    true,
                )?;
                let mut proxy_connection_write = proxy_connection_write.lock().await;
                proxy_connection_write.send(agent_message).await?;
            }
            let agent_relay_complete_message = PpaassMessageGenerator::create_tcp_session_relay_complete(
                user_token.as_ref(),
                session_key_a2p.as_ref(),
                src_address.clone(),
                dest_address.clone(),
                payload_encryption.clone(),
                true,
            )?;
            let mut proxy_connection_write = proxy_connection_write.lock().await;
            proxy_connection_write.send(agent_relay_complete_message).await?;
            debug!("Tcp session [{session_key_a2p}] read client data complete for proxy connection: [{pooled_proxy_connection_id_a2p}]");
            Ok::<_, anyhow::Error>(())
        });

        let p2a_guard = tokio::spawn(async move {
            loop {
                let proxy_data = {
                    let mut proxy_connection_read = proxy_connection_read.lock().await;
                    let Some(proxy_data) = proxy_connection_read.next().await else{
                        debug!("Tcp session [{session_key_p2a}] nothing to read from proxy connection [{pooled_proxy_connection_id_p2a}]");
                        break;
                    };
                    proxy_data
                };
                let proxy_data = proxy_data?;
                let PpaassMessageParts { payload_bytes, .. } = proxy_data.split();
                let PpaassMessagePayloadParts { payload_type, data } = TryInto::<PpaassMessagePayload>::try_into(payload_bytes)?.split();
                match payload_type {
                    PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionRelay) => {
                        let tcp_session_relay: TcpSessionRelayPayload = data.try_into()?;
                        let tcp_session_relay_status = tcp_session_relay.status;
                        let tcp_session_key = tcp_session_relay.session_key;
                        if session_key_p2a != tcp_session_key {
                            return Err(anyhow::anyhow!(format!(
                                "Tcp session [{session_key_p2a}] read proxy data from different tcp session [{tcp_session_key}] for proxy connection [{pooled_proxy_connection_id_p2a}]"
                            )));
                        }
                        let tcp_session_relay_data = tcp_session_relay.data;
                        match tcp_session_relay_status {
                            TcpSessionRelayStatus::Data => {
                                debug!(
                                    "Tcp session [{session_key_p2a}] read proxy data from proxy connection [{pooled_proxy_connection_id_p2a}]:\n{}\n.",
                                    pretty_hex::pretty_hex(&tcp_session_relay_data)
                                );
                                client_relay_framed_write.send(BytesMut::from_iter(tcp_session_relay_data)).await?;
                            },
                            TcpSessionRelayStatus::Complete => {
                                debug!("Tcp session [{session_key_p2a}] read proxy data complete for proxy connection [{pooled_proxy_connection_id_p2a}]");
                                break;
                            },
                        }
                    },
                    payload_type => {
                        error!("Tcp session [{session_key_p2a}] fail to read proxy data for proxy connection [{pooled_proxy_connection_id_p2a}] because of invalid payload type: {payload_type:?}");
                        return Err(anyhow::anyhow!(format!(
                            "Tcp session [{session_key_p2a}] fail to read proxy data for proxy connection [{pooled_proxy_connection_id_p2a}] because of invalid payload type: {payload_type:?}"
                        )));
                    },
                }
            }
            debug!("Tcp session [{session_key_p2a}] read proxy data complete for proxy connectoion [{pooled_proxy_connection_id_p2a}]");
            Ok::<_, anyhow::Error>(())
        });

        try_join!(a2p_guard, p2a_guard).map(|_| ()).map_err(|e| PooledProxyConnectionError {
            pooled_proxy_connection: Some(pooled_proxy_connection),
            source: anyhow::anyhow!(e),
        })
    }

    pub(crate) async fn exec(
        &mut self, proxy_connection_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>,
        rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<(), PooledProxyConnectionError> {
        let client_io = self
            .stream
            .take()
            .context("Fail to get client io stream")
            .map_err(|e| PooledProxyConnectionError {
                source: e,
                pooled_proxy_connection: None,
            })?;

        let mut auth_framed_parts = FramedParts::new(client_io, Socks5AuthCommandContentCodec);
        let mut auth_initial_buf = BytesMut::new();
        auth_initial_buf.put_u8(5);
        auth_framed_parts.read_buf = auth_initial_buf;
        let mut auth_framed = Framed::from_parts(auth_framed_parts);
        let auth_message = match auth_framed.next().await {
            None => return Ok(()),
            Some(result) => match result {
                Ok(result) => result,
                Err(e) => {
                    return Err(PooledProxyConnectionError {
                        source: e,
                        pooled_proxy_connection: None,
                    });
                },
            },
        };
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Socks5 connection in authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        if let Err(e) = auth_framed.send(auth_response).await {
            return Err(PooledProxyConnectionError {
                source: e,
                pooled_proxy_connection: None,
            });
        };
        let FramedParts { io: client_io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec);
        let init_message = match init_framed.next().await {
            None => return Ok(()),
            Some(result) => match result {
                Ok(result) => result,
                Err(e) => {
                    return Err(PooledProxyConnectionError {
                        source: e,
                        pooled_proxy_connection: None,
                    });
                },
            },
        };
        let Socks5InitCommandContentParts { request_type, dest_address } = init_message.split();
        debug!("Socks5 connection in init process, request type: {request_type:?}, destination address: {dest_address:?}");
        let user_token = match configuration
            .get_user_token()
            .as_ref()
            .context("Can not get user token form configuration file")
        {
            Ok(user_token) => user_token.clone(),
            Err(e) => {
                return Err(PooledProxyConnectionError {
                    source: e,
                    pooled_proxy_connection: None,
                });
            },
        };
        let src_address: PpaassNetAddress = self.client_socket_address.into();
        let dest_address: PpaassNetAddress = dest_address.into();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let pooled_proxy_connection: PooledProxyConnection = match proxy_connection_pool.get().await {
            Ok(v) => v,
            Err(e) => {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(e),
                    pooled_proxy_connection: None,
                });
            },
        };

        let tcp_session_init_request = match PpaassMessageGenerator::create_agent_tcp_session_initialize_request(
            &user_token,
            src_address.clone(),
            dest_address.clone(),
            payload_encryption.clone(),
        ) {
            Ok(v) => v,
            Err(e) => {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(e),
                    pooled_proxy_connection: Some(pooled_proxy_connection),
                });
            },
        };

        let proxy_connection_read = pooled_proxy_connection.clone_reader();
        let proxy_connection_write = pooled_proxy_connection.clone_writer();

        {
            let mut proxy_connection_write = proxy_connection_write.lock().await;
            if let Err(e) = proxy_connection_write.send(tcp_session_init_request).await {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(e),
                    pooled_proxy_connection: Some(pooled_proxy_connection),
                });
            };
        }
        let proxy_message = {
            let mut proxy_connection_read = proxy_connection_read.lock().await;
            let proxy_message = match proxy_connection_read.next().await {
                Some(v) => v,
                None => {
                    return Err(PooledProxyConnectionError {
                        source: anyhow::anyhow!("Can not read tcp session initialize response from proxy becuase of nothing to read."),
                        pooled_proxy_connection: Some(pooled_proxy_connection),
                    });
                },
            };
            match proxy_message {
                Ok(v) => v,
                Err(e) => {
                    return Err(PooledProxyConnectionError {
                        source: anyhow::anyhow!(e),
                        pooled_proxy_connection: Some(pooled_proxy_connection),
                    });
                },
            }
        };

        let PpaassMessageParts {
            id: proxy_message_id,
            payload_bytes: proxy_message_payload_bytes,
            user_token,
            ..
        } = proxy_message.split();
        let PpaassMessagePayloadParts { payload_type, data } = match TryInto::<PpaassMessagePayload>::try_into(proxy_message_payload_bytes) {
            Ok(v) => v,
            Err(e) => {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(e),
                    pooled_proxy_connection: Some(pooled_proxy_connection),
                });
            },
        }
        .split();
        let tcp_session_init_response = match payload_type {
            PpaassMessagePayloadType::AgentPayload(payload_type) => {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(format!("Invalid proxy payload type: {payload_type:?}")),
                    pooled_proxy_connection: Some(pooled_proxy_connection),
                });
            },
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeFail) => {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(format!("Fail to initialize tcp session becaise of failure response status")),
                    pooled_proxy_connection: Some(pooled_proxy_connection),
                });
            },
            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeSuccess) => {
                match TryInto::<TcpSessionInitResponsePayload>::try_into(data) {
                    Ok(v) => v,
                    Err(e) => {
                        return Err(PooledProxyConnectionError {
                            source: e,
                            pooled_proxy_connection: Some(pooled_proxy_connection),
                        });
                    },
                }
            },
            PpaassMessagePayloadType::ProxyPayload(payload_type) => {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(format!("Invalid proxy payload type: {payload_type:?}")),
                    pooled_proxy_connection: Some(pooled_proxy_connection),
                });
            },
        };
        debug!("Success init tcp session: {:?}", tcp_session_init_response.session_key);
        let socks5_init_success_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dest_address.clone().into()));
        if let Err(e) = init_framed.send(socks5_init_success_result).await {
            return Err(PooledProxyConnectionError {
                source: e,
                pooled_proxy_connection: Some(pooled_proxy_connection),
            });
        };
        let FramedParts { io: client_io, .. } = init_framed.into_parts();
        let session_key = match tcp_session_init_response.session_key.context("No session key assigend") {
            Ok(v) => v,
            Err(e) => {
                return Err(PooledProxyConnectionError {
                    source: anyhow::anyhow!(e),
                    pooled_proxy_connection: Some(pooled_proxy_connection),
                })
            },
        };
        debug!("Begin to relay socks5 data for tcp session: {session_key:?}");
        Self::relay(
            client_io,
            user_token,
            session_key,
            src_address,
            dest_address,
            payload_encryption,
            pooled_proxy_connection,
        )
        .await?;
        Ok(())
    }
}
