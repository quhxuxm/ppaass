pub(crate) mod dispatcher;

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Result;

use bytes::BytesMut;
use futures::{try_join, SinkExt, StreamExt};
use ppaass_common::{PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryption};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::timeout,
};

use tokio_util::codec::{BytesCodec, Framed};
use tracing::{debug, error, trace};

use crate::{
    config::AgentServerConfig,
    pool::{ProxyConnectionPool, ProxyConnectionRead, ProxyConnectionWrite},
};

use self::{http::HttpFlow, socks::Socks5Flow};

mod http;
mod socks;

struct ClientDataRelayInfo<T>
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
    init_data: Option<Vec<u8>>,
}

pub(crate) enum ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    Http {
        client_io: T,
        client_socket_address: SocketAddr,
        initial_buf: BytesMut,
    },
    Socks5 {
        client_io: T,
        client_socket_address: SocketAddr,
        initial_buf: BytesMut,
    },
}

impl<T> ClientFlow<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    pub(crate) async fn exec(self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>) -> Result<()> {
        match self {
            ClientFlow::Http {
                client_io,
                client_socket_address,
                initial_buf,
            } => {
                let http_flow = HttpFlow::new(client_io, client_socket_address);
                if let Err(e) = http_flow.exec(proxy_connection_pool, configuration, initial_buf).await {
                    error!("Client tcp connection [{client_socket_address}] error happen on http flow for proxy connection: {e:?}");
                    return Err(e);
                }
            },
            ClientFlow::Socks5 {
                client_io,
                client_socket_address,
                initial_buf,
            } => {
                let socks5_flow = Socks5Flow::new(client_io, client_socket_address);
                if let Err(e) = socks5_flow.exec(proxy_connection_pool, configuration, initial_buf).await {
                    error!("Client tcp connection [{client_socket_address}] error happen on socks5 flow for proxy connection: {e:?}");
                    return Err(e);
                };
            },
        }
        Ok(())
    }

    async fn relay(info: ClientDataRelayInfo<T>) -> Result<()> {
        let client_io = info.client_io;
        let tcp_loop_key = info.tcp_loop_key;
        let client_socket_address = info.client_socket_address;
        let configuration = info.configuration;
        let payload_encryption = info.payload_encryption;
        let mut proxy_connection_write = info.proxy_connection_write;
        let user_token = info.user_token;
        let mut proxy_connection_read = info.proxy_connection_read;
        let client_io_framed = Framed::with_capacity(client_io, BytesCodec::new(), configuration.get_client_io_buffer_size());
        let (mut client_io_write, mut client_io_read) = client_io_framed.split::<BytesMut>();
        let tcp_loop_key_a2p = tcp_loop_key.clone();
        let init_data = info.init_data;

        let mut client_to_proxy_relay_guard = tokio::spawn(async move {
            debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] start to relay from agent to proxy.");
            if let Some(init_data) = init_data {
                let agent_message = PpaassMessageGenerator::generate_raw_data(&user_token, payload_encryption.clone(), init_data)?;
                if let Err(e) = proxy_connection_write.send(agent_message).await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to relay client data to proxy because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }
            loop {
                let client_message = match timeout(Duration::from_secs(configuration.get_client_read_timeout()), client_io_read.next()).await {
                    Err(_) => {
                        error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] fail to read from client because of timeout");
                        return Err(anyhow::anyhow!(format!(
                            "Client tcp connection [{}] for tcp loop [{}] fail to read from client because of timeout",
                            client_socket_address, tcp_loop_key_a2p
                        )));
                    },
                    Ok(None) => {
                        debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_a2p}] complete to relay from client to proxy.");
                        break;
                    },
                    Ok(Some(Ok(client_message))) => client_message,
                    Ok(Some(Err(e))) => {
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
                    if let Err(e) = client_io_write.close().await {
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
                if let Err(e) = client_io_write.send(BytesMut::from_iter(proxy_message_payload_bytes)).await {
                    error!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to relay to proxy because of error: {e:?}");
                    if let Err(e) = client_io_write.close().await {
                        error!(
                            "Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] fail to shutdown client io because of error: {e:?}"
                        );
                    }
                    return Err(anyhow::anyhow!(e));
                };
            }
            debug!("Client tcp connection [{client_socket_address}] for tcp loop [{tcp_loop_key_p2a}] complete to relay from proxy to agent.");
            if let Err(e) = client_io_write.close().await {
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
}
