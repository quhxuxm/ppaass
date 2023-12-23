pub(crate) mod dispatcher;
mod http;
mod socks;

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use self::dispatcher::ClientTransportHandshakeInfo;
use crate::{
    config::AGENT_CONFIG,
    crypto::AgentServerRsaCryptoFetcher,
    error::{AgentError, NetworkError},
    AgentServerPayloadEncryptionTypeSelector,
};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures::StreamExt as FuturesStreamExt;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream,
};
use log::{debug, error};

use pin_project::pin_project;
use ppaass_common::tcp::ProxyTcpPayload;
use ppaass_common::{
    proxy::PpaassProxyConnection, random_32_bytes, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassProxyMessage,
    PpaassProxyMessagePayload, PpaassUnifiedAddress,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, UdpSocket},
};
use tokio_stream::StreamExt as TokioStreamExt;
use tokio_util::codec::{BytesCodec, Framed};

#[pin_project]
struct ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    src_address: PpaassUnifiedAddress,

    #[pin]
    client_bytes_framed_write: SplitSink<Framed<T, BytesCodec>, BytesMut>,
}

impl<T> ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn new(src_address: PpaassUnifiedAddress, client_bytes_framed_write: SplitSink<Framed<T, BytesCodec>, BytesMut>) -> Self {
        Self {
            src_address,
            client_bytes_framed_write,
        }
    }
}

impl<T> Sink<BytesMut> for ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    type Error = NetworkError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_ready(cx).map_err(NetworkError::General)
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        this.client_bytes_framed_write.start_send(item).map_err(NetworkError::General)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_flush(cx).map_err(NetworkError::General)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_close(cx).map_err(NetworkError::General)
    }
}

#[pin_project]
struct ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    src_address: PpaassUnifiedAddress,
    #[pin]
    client_bytes_framed_read: SplitStream<Framed<T, BytesCodec>>,
}

impl<T> ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn new(src_address: PpaassUnifiedAddress, client_bytes_framed_read: SplitStream<Framed<T, BytesCodec>>) -> Self {
        Self {
            src_address,
            client_bytes_framed_read,
        }
    }
}

impl<T> Stream for ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    type Item = Result<BytesMut, NetworkError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.client_bytes_framed_read.poll_next(cx).map_err(NetworkError::General)
    }
}

pub(crate) enum ClientTransportDataRelayInfo {
    Tcp(ClientTransportTcpDataRelay),
    Udp(ClientTransportUdpDataRelay),
}

#[non_exhaustive]
pub(crate) struct ClientTransportTcpDataRelay {
    tunnel_id: String,
    client_tcp_stream: TcpStream,
    src_address: PpaassUnifiedAddress,
    dst_address: PpaassUnifiedAddress,
    proxy_connection: PpaassProxyConnection<AgentServerRsaCryptoFetcher>,
    init_data: Option<Bytes>,
}

#[non_exhaustive]
pub(crate) struct ClientTransportUdpDataRelay {
    client_tcp_stream: TcpStream,
    agent_udp_bind_socket: UdpSocket,
    client_udp_restrict_address: PpaassUnifiedAddress,
}

#[async_trait]
pub(crate) trait ClientTransportHandshake {
    async fn handshake(
        &self, handshake_info: ClientTransportHandshakeInfo,
    ) -> Result<(ClientTransportDataRelayInfo, Box<dyn ClientTransportRelay + Send + Sync>), AgentError>;
}

#[async_trait]
pub(crate) trait ClientTransportRelay {
    async fn udp_relay(&self, _udp_relay_info: ClientTransportUdpDataRelay) -> Result<(), AgentError> {
        Ok(())
    }

    async fn tcp_relay(&self, tcp_relay_info: ClientTransportTcpDataRelay) -> Result<(), AgentError> {
        let user_token = AGENT_CONFIG.get_user_token();
        let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(user_token, Some(random_32_bytes()));
        let ClientTransportTcpDataRelay {
            tunnel_id,
            client_tcp_stream,
            src_address,
            dst_address,
            mut proxy_connection,
            init_data,
        } = tcp_relay_info;

        debug!("Agent going to relay tcp data from destination: {dst_address}");

        let proxy_relay_timeout = AGENT_CONFIG.get_proxy_relay_timeout();
        let client_relay_timeout = AGENT_CONFIG.get_client_relay_timeout();
        let client_io_framed = Framed::with_capacity(client_tcp_stream, BytesCodec::new(), AGENT_CONFIG.get_client_receive_buffer_size());
        let (client_io_write, client_io_read) = client_io_framed.split::<BytesMut>();
        let (mut client_io_write, client_io_read) = (
            ClientConnectionWrite::new(src_address.clone(), client_io_write),
            ClientConnectionRead::new(src_address.clone(), client_io_read),
        );
        if let Some(init_data) = init_data {
            let agent_message = PpaassMessageGenerator::generate_agent_tcp_data_message(user_token.to_string(), payload_encryption.clone(), init_data)?;
            proxy_connection.send(agent_message).await?;
        }

        let (mut proxy_connection_write, proxy_connection_read) = proxy_connection.split();

        {
            let tunnel_id = tunnel_id.clone();
            let dst_address = dst_address.clone();
            tokio::spawn(async move {
                // Forward client data to proxy
                if let Err(e) = TokioStreamExt::map_while(client_io_read.timeout(Duration::from_secs(client_relay_timeout)), |client_message| {
                    let client_message = client_message.ok()?;
                    let client_message = client_message.ok()?;
                    let tcp_data =
                        PpaassMessageGenerator::generate_agent_tcp_data_message(user_token.to_string(), payload_encryption.clone(), client_message.freeze())
                            .ok()?;
                    Some(Ok(tcp_data))
                })
                .forward(&mut proxy_connection_write)
                .await
                {
                    error!("Tunnel {tunnel_id} error happen when relay tcp data from client to proxy for destination [{dst_address}], error: {e:?}");
                }
            });
        }

        tokio::spawn(async move {
            if let Err(e) = TokioStreamExt::map_while(proxy_connection_read.timeout(Duration::from_secs(proxy_relay_timeout)), |proxy_message| {
                let proxy_message = proxy_message.ok()?;
                let PpaassProxyMessage {
                    payload: PpaassProxyMessagePayload::Tcp(ProxyTcpPayload::Data { content }),
                    ..
                } = proxy_message.ok()?
                else {
                    error!("Fail to parse proxy message payload because of not a tcp data");
                    return None;
                };
                Some(Ok(BytesMut::from_iter(content)))
            })
            .forward(&mut client_io_write)
            .await
            {
                error!("Tunnel {tunnel_id} error happen when relay tcp data from proxy to client for destination [{dst_address}], error: {e:?}",);
            }
        });

        Ok(())
    }
}
