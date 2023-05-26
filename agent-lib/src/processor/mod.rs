pub(crate) mod dispatcher;

use std::{
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use self::{http::HttpClientProcessor, socks::Socks5ClientProcessor};
use crate::{
    config::AgentServerConfig,
    error::{AgentError, NetworkError},
    pool::ProxyConnectionPool,
};

use bytes::BytesMut;
use futures::StreamExt as FuturesStreamExt;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream,
};
use pin_project::pin_project;
use ppaass_common::{
    tcp::TcpData, CommonError, PpaassConnectionRead, PpaassConnectionWrite, PpaassMessage, PpaassMessageGenerator, PpaassMessagePayloadEncryption,
    PpaassNetAddress, RsaCryptoFetcher,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_stream::StreamExt as TokioStreamExt;
use tokio_util::codec::{BytesCodec, Framed};
use tracing::error;

mod http;
mod socks;

#[non_exhaustive]
struct ClientDataRelayInfo<R, I>
where
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: ToString + Send + Sync + Clone + Display + Debug + 'static,
{
    client_tcp_stream: TcpStream,
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    user_token: String,
    payload_encryption: PpaassMessagePayloadEncryption,
    proxy_connection_read: PpaassConnectionRead<TcpStream, R, I>,
    proxy_connection_write: PpaassConnectionWrite<TcpStream, R, I>,
    configuration: Arc<AgentServerConfig>,
    init_data: Option<Vec<u8>>,
}

pub(crate) enum ClientProtocolProcessor {
    Http {
        client_tcp_stream: TcpStream,
        src_address: PpaassNetAddress,
        initial_buf: BytesMut,
    },
    Socks5 {
        client_tcp_stream: TcpStream,
        src_address: PpaassNetAddress,
        initial_buf: BytesMut,
    },
}

impl ClientProtocolProcessor {
    pub(crate) async fn exec(self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>) -> Result<(), AgentError> {
        match self {
            ClientProtocolProcessor::Http {
                client_tcp_stream,
                src_address,
                initial_buf,
            } => {
                let http_flow = HttpClientProcessor::new(client_tcp_stream, src_address.clone());
                http_flow.exec(proxy_connection_pool, configuration, initial_buf).await?;
            },
            ClientProtocolProcessor::Socks5 {
                client_tcp_stream,
                src_address,
                initial_buf,
            } => {
                let socks5_flow = Socks5ClientProcessor::new(client_tcp_stream, src_address.clone());
                socks5_flow.exec(proxy_connection_pool, configuration, initial_buf).await?;
            },
        }
        Ok(())
    }

    async fn relay<R, I>(info: ClientDataRelayInfo<R, I>) -> Result<(), AgentError>
    where
        R: RsaCryptoFetcher + Send + Sync + 'static,
        I: ToString + Send + Sync + Clone + Display + Debug + 'static,
    {
        let client_tcp_stream = info.client_tcp_stream;

        let src_address = info.src_address;
        let dst_address = info.dst_address;
        let configuration = info.configuration;
        let proxy_relay_timeout = configuration.get_proxy_relay_timeout();
        let client_relay_timeout = configuration.get_client_relay_timeout();
        let payload_encryption = info.payload_encryption;
        let mut proxy_connection_write = info.proxy_connection_write;
        let user_token = info.user_token;
        let proxy_connection_read = info.proxy_connection_read;
        let client_io_framed = Framed::with_capacity(client_tcp_stream, BytesCodec::new(), configuration.get_client_receive_buffer_size());
        let (client_io_write, client_io_read) = client_io_framed.split::<BytesMut>();
        let (mut client_io_write, client_io_read) = (
            ClientConnectionWrite::new(src_address.clone(), client_io_write),
            ClientConnectionRead::new(src_address.clone(), client_io_read),
        );

        let init_data = info.init_data;

        if let Some(init_data) = init_data {
            let agent_message =
                PpaassMessageGenerator::generate_tcp_data(&user_token, payload_encryption.clone(), src_address.clone(), dst_address.clone(), init_data)?;
            proxy_connection_write.send(agent_message).await?;
        }

        tokio::spawn(async move {
            if let Err(e) = TokioStreamExt::map_while(client_io_read.timeout(Duration::from_secs(client_relay_timeout)), |client_message| {
                let client_message = client_message.ok()?;
                let client_message = client_message.ok()?;
                let tcp_data = PpaassMessageGenerator::generate_tcp_data(
                    user_token.clone(),
                    payload_encryption.clone(),
                    src_address.clone(),
                    dst_address.clone(),
                    client_message.to_vec(),
                )
                .ok()?;
                Some(Ok(tcp_data))
            })
            .forward(&mut proxy_connection_write)
            .await
            {
                error!("Client connection fail to relay client data to proxy because of error: {e:?}");
                proxy_connection_write.flush().await?;
                proxy_connection_write.close().await?;
            }
            Ok::<_, CommonError>(())
        });

        tokio::spawn(async move {
            if let Err(e) = TokioStreamExt::map_while(proxy_connection_read.timeout(Duration::from_secs(proxy_relay_timeout)), |proxy_message| {
                let proxy_message = proxy_message.ok()?;
                let PpaassMessage { payload, .. } = proxy_message.ok()?;
                let TcpData { data, .. } = payload.as_slice().try_into().ok()?;
                Some(Ok(BytesMut::from_iter(data)))
            })
            .forward(&mut client_io_write)
            .await
            {
                error!("Client connection fail to relay proxy data to client because of error: {e:?}");
                client_io_write.flush().await?;
                client_io_write.close().await?;
            }
            Ok::<_, AgentError>(())
        });

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
    type Error = AgentError;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_ready(cx).map_err(|e| NetworkError::Io(e).into())
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        this.client_bytes_framed_write.start_send(item).map_err(|e| NetworkError::Io(e).into())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_flush(cx).map_err(|e| NetworkError::Io(e).into())
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_close(cx).map_err(|e| NetworkError::Io(e).into())
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
    type Item = Result<BytesMut, AgentError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.client_bytes_framed_read.poll_next(cx).map_err(|e| NetworkError::Io(e).into())
    }
}
