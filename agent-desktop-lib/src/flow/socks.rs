use bytes::{BufMut, BytesMut};
use deadpool::managed::{Object, Pool};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
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

#[derive(Debug, Clone, Copy)]
pub(crate) enum Socks5FlowStatus {
    New,
    Authenticate,
    InitConnect,
    InitBind,
    InitUdpAssociate,
    Relay,
}

pub(crate) struct Socks5FlowError<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) status: Socks5FlowStatus,
    pub(crate) client_stream: Option<T>,
    pub(crate) proxy_connection: Option<Object<ProxyConnectionManager>>,
    pub(crate) source: anyhow::Error,
}

struct Socks5RelayAgentToProxyError<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    client_relay_framed_read: SplitStream<Framed<T, BytesCodec>>,
    source: anyhow::Error,
}

struct Socks5RelayProxyToAgentError<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    client_relay_framed_write: SplitSink<Framed<T, BytesCodec>, BytesMut>,
    source: anyhow::Error,
}
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
        payload_encryption: PpaassMessagePayloadEncryption, proxy_connection: Object<ProxyConnectionManager>,
    ) -> Result<(), Socks5FlowError<T>>
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
                        return Err(Socks5RelayAgentToProxyError {
                            client_relay_framed_read,
                            source: anyhow::anyhow!(e),
                        });
                    },
                    Ok(client_data) => {
                        info!(
                            "Read client data going to send to proxy connection [{:?}]: \n{}\n",
                            proxy_connection_id,
                            pretty_hex::pretty_hex(&client_data)
                        );
                        let agent_message = match PpaassMessageUtil::create_tcp_session_relay(
                            user_token.as_ref(),
                            session_key.as_ref(),
                            src_address.clone(),
                            dest_address.clone(),
                            payload_encryption.clone(),
                            client_data.to_vec(),
                            true,
                        ) {
                            Ok(agent_message) => agent_message,
                            Err(e) => {
                                return Err(Socks5RelayAgentToProxyError {
                                    client_relay_framed_read,
                                    source: anyhow::anyhow!(e),
                                });
                            },
                        };
                        let mut proxy_connection_write = proxy_connection_write.lock().await;
                        if let Err(e) = proxy_connection_write.send(agent_message).await {
                            if let Err(e) = proxy_connection_write.close().await {
                                return Err(Socks5RelayAgentToProxyError {
                                    client_relay_framed_read,
                                    source: anyhow::anyhow!(e),
                                });
                            }
                            return Err(Socks5RelayAgentToProxyError {
                                client_relay_framed_read,
                                source: anyhow::anyhow!(e),
                            });
                        };
                    },
                }
            }
            Ok::<_, Socks5RelayAgentToProxyError<T>>(client_relay_framed_read)
        });
        let proxy_to_agnet_relay_guard = tokio::spawn(async move {
            let mut proxy_connection_read = proxy_connection_read.lock().await;
            while let Some(proxy_data) = proxy_connection_read.next().await {
                match proxy_data {
                    Err(e) => {
                        error!("Fail to read proxy data because of error: {e:?}");
                        return Err(Socks5RelayProxyToAgentError {
                            client_relay_framed_write,
                            source: anyhow::anyhow!(e),
                        });
                    },
                    Ok(proxy_data) => {
                        let PpaassMessageParts { payload_bytes, .. } = proxy_data.split();
                        let PpaassMessagePayloadParts { payload_type, data } = match TryInto::<PpaassMessagePayload>::try_into(payload_bytes) {
                            Ok(v) => v,
                            Err(e) => {
                                return Err(Socks5RelayProxyToAgentError {
                                    client_relay_framed_write,
                                    source: anyhow::anyhow!(e),
                                });
                            },
                        }
                        .split();
                        match payload_type {
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionRelay) => {
                                let tcp_session_relay: TcpSessionRelayPayload = match data.try_into() {
                                    Ok(tcp_session_relay) => tcp_session_relay,
                                    Err(e) => {
                                        return Err(Socks5RelayProxyToAgentError {
                                            client_relay_framed_write,
                                            source: anyhow::anyhow!(e),
                                        });
                                    },
                                };
                                if let Err(e) = client_relay_framed_write.send(BytesMut::from_iter(tcp_session_relay.data)).await {
                                    return Err(Socks5RelayProxyToAgentError {
                                        client_relay_framed_write,
                                        source: anyhow::anyhow!(e),
                                    });
                                };
                            },
                            payload_type => {
                                error!("Fail to read proxy data because of invalid payload type: {payload_type:?}");
                                return Err(Socks5RelayProxyToAgentError {
                                    client_relay_framed_write,
                                    source: anyhow::anyhow!(format!("Fail to read proxy data because of invalid payload type: {payload_type:?}")),
                                });
                            },
                        }
                    },
                }
            }
            Ok::<_, Socks5RelayProxyToAgentError<T>>(client_relay_framed_write)
        });
        let (a2p, p2a) = join!(agnet_to_proxy_relay_guard, proxy_to_agnet_relay_guard);
        match (a2p, p2a) {
            (Ok(Ok(_)), Ok(Ok(_))) => return Ok(()),
            (
                Ok(Err(Socks5RelayAgentToProxyError {
                    client_relay_framed_read,
                    source,
                })),
                Ok(Ok(client_relay_framed_write)),
            ) => {
                let client_relay_framed = match client_relay_framed_read.reunite(client_relay_framed_write) {
                    Ok(client_relay_framed) => client_relay_framed,
                    Err(e) => {
                        return Err(Socks5FlowError {
                            status: Socks5FlowStatus::Relay,
                            client_stream: None,
                            proxy_connection: Some(proxy_connection),
                            source: anyhow::anyhow!(e),
                        });
                    },
                };
                let FramedParts { io, .. } = client_relay_framed.into_parts();
                Err(Socks5FlowError {
                    status: Socks5FlowStatus::Relay,
                    client_stream: Some(io),
                    proxy_connection: Some(proxy_connection),
                    source,
                })
            },
            (
                Ok(Err(Socks5RelayAgentToProxyError {
                    client_relay_framed_read,
                    source: source_a2p,
                })),
                Ok(Err(Socks5RelayProxyToAgentError {
                    client_relay_framed_write,
                    source: source_p2a,
                })),
            ) => {
                error!("Agent to proxy relay has error: {source_a2p:?}");
                error!("Proxy to agent relay has error: {source_p2a:?}");
                let client_relay_framed = match client_relay_framed_read.reunite(client_relay_framed_write) {
                    Ok(client_relay_framed) => client_relay_framed,
                    Err(e) => {
                        return Err(Socks5FlowError {
                            status: Socks5FlowStatus::Relay,
                            client_stream: None,
                            proxy_connection: Some(proxy_connection),
                            source: anyhow::anyhow!(e),
                        });
                    },
                };
                let FramedParts { io, .. } = client_relay_framed.into_parts();
                Err(Socks5FlowError {
                    status: Socks5FlowStatus::Relay,
                    client_stream: Some(io),
                    proxy_connection: Some(proxy_connection),
                    source: anyhow::anyhow!("Both agent to proxy relay and proxy to agent relay have error"),
                })
            },
            (Ok(Ok(_client_relay_framed_read)), Err(join_error)) => Err(Socks5FlowError {
                status: Socks5FlowStatus::Relay,
                client_stream: None,
                proxy_connection: Some(proxy_connection),
                source: anyhow::anyhow!(join_error),
            }),
            (Ok(Err(Socks5RelayAgentToProxyError { source, .. })), Err(join_error)) => {
                error!("Agent to proxy relay has error: {source:?}");
                Err(Socks5FlowError {
                    status: Socks5FlowStatus::Relay,
                    client_stream: None,
                    proxy_connection: Some(proxy_connection),
                    source: anyhow::anyhow!(join_error),
                })
            },
            (Err(join_error), Ok(Ok(_))) => Err(Socks5FlowError {
                status: Socks5FlowStatus::Relay,
                client_stream: None,
                proxy_connection: Some(proxy_connection),
                source: anyhow::anyhow!(join_error),
            }),
            (Err(join_error), Ok(Err(Socks5RelayProxyToAgentError { source, .. }))) => {
                error!("Proxy to agent relay has error: {source:?}");
                Err(Socks5FlowError {
                    status: Socks5FlowStatus::Relay,
                    client_stream: None,
                    proxy_connection: Some(proxy_connection),
                    source: anyhow::anyhow!(join_error),
                })
            },
            (Err(join_error_a2p), Err(join_error_p2a)) => {
                error!("Agent to proxy relay has error: {join_error_a2p:?}");
                error!("Proxy to agent relay has error: {join_error_p2a:?}");
                Err(Socks5FlowError {
                    status: Socks5FlowStatus::Relay,
                    client_stream: None,
                    proxy_connection: Some(proxy_connection),
                    source: anyhow::anyhow!("Both agent to proxy relay and proxy to agent relay have error."),
                })
            },
            (
                Ok(Ok(client_relay_framed_read)),
                Ok(Err(Socks5RelayProxyToAgentError {
                    client_relay_framed_write,
                    source,
                })),
            ) => {
                let client_relay_framed = match client_relay_framed_read.reunite(client_relay_framed_write) {
                    Ok(client_relay_framed) => client_relay_framed,
                    Err(e) => {
                        return Err(Socks5FlowError {
                            status: Socks5FlowStatus::Relay,
                            client_stream: None,
                            proxy_connection: Some(proxy_connection),
                            source: anyhow::anyhow!(e),
                        });
                    },
                };
                let FramedParts { io, .. } = client_relay_framed.into_parts();
                Err(Socks5FlowError {
                    status: Socks5FlowStatus::Relay,
                    client_stream: Some(io),
                    proxy_connection: Some(proxy_connection),
                    source,
                })
            },
        }
    }

    pub(crate) async fn exec(
        &mut self, proxy_connection_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>,
        rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<(), Socks5FlowError<T>> {
        let client_io = self.stream.take().context("Fail to get client io stream").map_err(|e| Socks5FlowError {
            source: e,
            client_stream: None,
            proxy_connection: None,
            status: Socks5FlowStatus::New,
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
                    let FramedParts { io, .. } = auth_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: e,
                        client_stream: Some(io),
                        proxy_connection: None,
                        status: Socks5FlowStatus::Authenticate,
                    });
                },
            },
        };
        let Socks5AuthCommandContentParts { methods } = auth_message.split();
        debug!("Socks5 connection in authenticate process, authenticate methods in request: {methods:?}");
        let auth_response = Socks5AuthCommandResultContent::new(message::Socks5AuthMethod::NoAuthenticationRequired);
        if let Err(e) = auth_framed.send(auth_response).await {
            let FramedParts { io, .. } = auth_framed.into_parts();
            return Err(Socks5FlowError {
                source: e,
                client_stream: Some(io),
                proxy_connection: None,
                status: Socks5FlowStatus::Authenticate,
            });
        };
        let FramedParts { io: client_io, .. } = auth_framed.into_parts();
        let mut init_framed = Framed::new(client_io, Socks5InitCommandContentCodec);
        let init_message = match init_framed.next().await {
            None => return Ok(()),
            Some(result) => match result {
                Ok(result) => result,
                Err(e) => {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: e,
                        client_stream: Some(io),
                        proxy_connection: None,
                        status: Socks5FlowStatus::InitConnect,
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
                let FramedParts { io, .. } = init_framed.into_parts();
                return Err(Socks5FlowError {
                    source: e,
                    client_stream: Some(io),
                    proxy_connection: None,
                    status: Socks5FlowStatus::InitConnect,
                });
            },
        };
        let src_address: PpaassNetAddress = self.client_socket_address.into();
        let dest_address: PpaassNetAddress = dest_address.into();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let proxy_connection = match proxy_connection_pool.get().await {
            Ok(proxy_connection) => proxy_connection,
            Err(e) => {
                let FramedParts { io, .. } = init_framed.into_parts();
                return Err(Socks5FlowError {
                    source: anyhow::anyhow!(e),
                    client_stream: Some(io),
                    proxy_connection: None,
                    status: Socks5FlowStatus::InitConnect,
                });
            },
        };
        let (tcp_session_init_response, client_io) = {
            let proxy_connection_read = proxy_connection.get_reader();
            let proxy_connection_write = proxy_connection.get_writer();

            let tcp_session_init_request = match PpaassMessageUtil::create_agent_tcp_session_initialize_request(
                &user_token,
                src_address.clone(),
                dest_address.clone(),
                payload_encryption.clone(),
            ) {
                Ok(tcp_session_init_request) => tcp_session_init_request,
                Err(e) => {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: anyhow::anyhow!(e),
                        client_stream: Some(io),
                        proxy_connection: Some(proxy_connection),
                        status: Socks5FlowStatus::InitConnect,
                    });
                },
            };

            {
                let mut proxy_connection_write = proxy_connection_write.lock().await;
                if let Err(e) = proxy_connection_write.send(tcp_session_init_request).await {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: anyhow::anyhow!(e),
                        client_stream: Some(io),
                        proxy_connection: Some(proxy_connection),
                        status: Socks5FlowStatus::InitConnect,
                    });
                };
            }
            let proxy_message = {
                let mut proxy_connection_read = proxy_connection_read.lock().await;

                let Some(proxy_message) = proxy_connection_read.next().await else {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: anyhow::anyhow!("Can not read tcp session initialize response from proxy becuase of nothing to read."),
                        client_stream: Some(io),
                          proxy_connection: Some(proxy_connection),
                        status: Socks5FlowStatus::InitConnect,
                    });
                };
                match proxy_message {
                    Ok(proxy_message) => proxy_message,
                    Err(e) => {
                        let FramedParts { io, .. } = init_framed.into_parts();
                        return Err(Socks5FlowError {
                            source: anyhow::anyhow!(e),
                            client_stream: Some(io),
                            proxy_connection: Some(proxy_connection),
                            status: Socks5FlowStatus::InitConnect,
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
                Ok(message_payload) => message_payload,
                Err(e) => {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: anyhow::anyhow!(e),
                        client_stream: Some(io),
                        proxy_connection: Some(proxy_connection),
                        status: Socks5FlowStatus::InitConnect,
                    });
                },
            }
            .split();
            let tcp_session_init_response = match payload_type {
                PpaassMessagePayloadType::AgentPayload(payload_type) => {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: anyhow::anyhow!(format!("Invalid proxy payload type: {payload_type:?}")),
                        client_stream: Some(io),
                        proxy_connection: Some(proxy_connection),
                        status: Socks5FlowStatus::InitConnect,
                    });
                },
                PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeFail) => {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: anyhow::anyhow!(format!("Fail to initialize tcp session becaise of failure response status")),
                        client_stream: Some(io),
                        proxy_connection: Some(proxy_connection),
                        status: Socks5FlowStatus::InitConnect,
                    });
                },
                PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpSessionInitializeSuccess) => {
                    let tcp_session_init_response: TcpSessionInitResponsePayload = match data.try_into() {
                        Ok(tcp_session_init_response) => tcp_session_init_response,
                        Err(e) => {
                            let FramedParts { io, .. } = init_framed.into_parts();
                            return Err(Socks5FlowError {
                                source: e,
                                client_stream: Some(io),
                                proxy_connection: Some(proxy_connection),
                                status: Socks5FlowStatus::InitConnect,
                            });
                        },
                    };
                    tcp_session_init_response
                },
                PpaassMessagePayloadType::ProxyPayload(payload_type) => {
                    let FramedParts { io, .. } = init_framed.into_parts();
                    return Err(Socks5FlowError {
                        source: anyhow::anyhow!(format!("Invalid proxy payload type: {payload_type:?}")),
                        client_stream: Some(io),
                        proxy_connection: Some(proxy_connection),
                        status: Socks5FlowStatus::InitConnect,
                    });
                },
            };
            debug!("Success init tcp session: {:?}", tcp_session_init_response.session_key);
            let socks5_init_success_result = Socks5InitCommandResultContent::new(Socks5InitCommandResultStatus::Succeeded, Some(dest_address.clone().into()));
            if let Err(e) = init_framed.send(socks5_init_success_result).await {
                let FramedParts { io, .. } = init_framed.into_parts();
                return Err(Socks5FlowError {
                    source: anyhow::anyhow!(e),
                    client_stream: Some(io),
                    proxy_connection: Some(proxy_connection),
                    status: Socks5FlowStatus::InitConnect,
                });
            };
            let FramedParts { io: client_io, .. } = init_framed.into_parts();
            debug!("Begin to relay socks5 data for tcp session: {:?}", tcp_session_init_response.session_key);
            (tcp_session_init_response, client_io)
        };
        let session_key = match tcp_session_init_response.session_key.context("No session key assigend") {
            Ok(session_key) => session_key,
            Err(e) => {
                return Err(Socks5FlowError {
                    source: anyhow::anyhow!(e),
                    client_stream: Some(client_io),
                    proxy_connection: Some(proxy_connection),
                    status: Socks5FlowStatus::InitConnect,
                })
            },
        };

        Self::relay(
            client_io,
            user_token,
            session_key,
            src_address,
            dest_address,
            payload_encryption,
            proxy_connection,
        )
        .await?;
        Ok(())
    }
}
