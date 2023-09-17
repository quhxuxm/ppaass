pub(crate) mod dispatcher;

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
};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use futures::StreamExt as FuturesStreamExt;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream,
};

use pin_project::pin_project;
use ppaass_common::{
    proxy::PpaassProxyConnection, tcp::AgentTcpData, PpaassMessageGenerator, PpaassMessagePayloadEncryption, PpaassNetAddress, PpaassProxyMessage,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_stream::StreamExt as TokioStreamExt;
use tokio_util::codec::{BytesCodec, Framed};

mod http;
mod socks;

#[non_exhaustive]
pub(crate) struct ClientTransportDataRelayInfo {
    client_tcp_stream: TcpStream,
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption,
    proxy_connection: PpaassProxyConnection<'static, TcpStream, AgentServerRsaCryptoFetcher, String>,
    init_data: Option<Bytes>,
}

#[async_trait]
pub(crate) trait ClientTransport {
    async fn handshake(&self, handshake_info: ClientTransportHandshakeInfo) -> Result<ClientTransportDataRelayInfo, AgentError>;

    async fn relay(&self, relay_info: ClientTransportDataRelayInfo) -> Result<(), AgentError> {
        let client_tcp_stream = relay_info.client_tcp_stream;
        let src_address = relay_info.src_address;
        let dst_address = relay_info.dst_address;
        let proxy_relay_timeout = AGENT_CONFIG.get_proxy_relay_timeout();
        let client_relay_timeout = AGENT_CONFIG.get_client_relay_timeout();
        let payload_encryption = relay_info.payload_encryption;
        let mut proxy_connection = relay_info.proxy_connection;
        let user_token = relay_info.user_token;
        let client_io_framed = Framed::with_capacity(client_tcp_stream, BytesCodec::new(), AGENT_CONFIG.get_client_receive_buffer_size());
        let (client_io_write, client_io_read) = client_io_framed.split::<BytesMut>();
        let (mut client_io_write, client_io_read) = (
            ClientConnectionWrite::new(src_address.clone(), client_io_write),
            ClientConnectionRead::new(src_address.clone(), client_io_read),
        );

        if let Some(init_data) = relay_info.init_data {
            let agent_message = PpaassMessageGenerator::generate_agent_tcp_data_message(
                &user_token,
                payload_encryption.clone(),
                src_address.clone(),
                dst_address.clone(),
                init_data,
            )?;
            proxy_connection.send(agent_message).await?;
        }

        let (mut proxy_connection_write, proxy_connection_read) = proxy_connection.split();

        let (_, _) = tokio::join!(
            TokioStreamExt::map_while(client_io_read.timeout(Duration::from_secs(client_relay_timeout)), |client_message| {
                let client_message = client_message.ok()?;
                let client_message = client_message.ok()?;
                let tcp_data = PpaassMessageGenerator::generate_agent_tcp_data_message(
                    user_token.clone(),
                    payload_encryption.clone(),
                    src_address.clone(),
                    dst_address.clone(),
                    client_message.freeze(),
                )
                .ok()?;
                Some(Ok(tcp_data))
            })
            .forward(&mut proxy_connection_write),
            TokioStreamExt::map_while(proxy_connection_read.timeout(Duration::from_secs(proxy_relay_timeout)), |proxy_message| {
                let proxy_message = proxy_message.ok()?;
                let PpaassProxyMessage { payload, .. } = proxy_message.ok()?;
                let AgentTcpData { data, .. } = payload.data.try_into().ok()?;
                Some(Ok(BytesMut::from_iter(data)))
            })
            .forward(&mut client_io_write)
        );

        Ok(())
    }
}

#[pin_project]
struct ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    src_address: PpaassNetAddress,

    #[pin]
    client_bytes_framed_write: SplitSink<Framed<T, BytesCodec>, BytesMut>,
}

impl<T> ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn new(src_address: PpaassNetAddress, client_bytes_framed_write: SplitSink<Framed<T, BytesCodec>, BytesMut>) -> Self {
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
    src_address: PpaassNetAddress,
    #[pin]
    client_bytes_framed_read: SplitStream<Framed<T, BytesCodec>>,
}

impl<T> ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn new(src_address: PpaassNetAddress, client_bytes_framed_read: SplitStream<Framed<T, BytesCodec>>) -> Self {
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
