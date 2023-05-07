pub(crate) mod dispatcher;

use std::{
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use anyhow::{anyhow, Result};

use bytes::BytesMut;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use pin_project::pin_project;
use ppaass_common::{
    tcp::{TcpData, TcpDataParts},
    PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryption, PpaassNetAddress,
    RsaCryptoFetcher,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use tokio_util::codec::{BytesCodec, Framed};
use tracing::{error, trace};

use crate::{config::AgentServerConfig, pool::ProxyConnectionPool};

use self::{http::HttpClientProcessor, socks::Socks5ClientProcessor};

mod http;
mod socks;

#[non_exhaustive]
struct ClientDataRelayInfo<R, I>
where
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    client_tcp_stream: TcpStream,
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    tcp_loop_key: String,
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
    pub(crate) async fn exec(self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>) -> Result<()> {
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

    async fn relay<R, I>(info: ClientDataRelayInfo<R, I>) -> Result<()>
    where
        R: RsaCryptoFetcher + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
    {
        let client_tcp_stream = info.client_tcp_stream;
        let tcp_loop_key = info.tcp_loop_key;
        let src_address = info.src_address;
        let dst_address = info.dst_address;
        let configuration = info.configuration;
        let payload_encryption = info.payload_encryption;
        let mut proxy_connection_write = info.proxy_connection_write;
        let user_token = info.user_token;
        let mut proxy_connection_read = info.proxy_connection_read;
        let client_io_framed = Framed::with_capacity(client_tcp_stream, BytesCodec::new(), configuration.get_client_receive_buffer_size());
        let (client_io_write, client_io_read) = client_io_framed.split::<BytesMut>();
        let (mut client_io_write, mut client_io_read) = (
            ClientConnectionWrite::new(src_address.clone(), client_io_write),
            ClientConnectionRead::new(src_address.clone(), client_io_read),
        );

        let init_data = info.init_data;

        if let Some(init_data) = init_data {
            let agent_message =
                PpaassMessageGenerator::generate_tcp_data(&user_token, payload_encryption.clone(), src_address.clone(), dst_address.clone(), init_data)?;
            if let Err(e) = proxy_connection_write.send(agent_message).await {
                error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to relay client data to proxy because of error: {e:?}");
                return Err(anyhow::anyhow!(e));
            };
        }
        let mut stop_read_client = false;
        let mut stop_read_proxy = false;
        loop {
            if stop_read_client && stop_read_proxy {
                break Ok(());
            }
            tokio::select! {
                client_message = client_io_read.next(), if !stop_read_client => {
                   let client_message = match client_message {
                    Some(Ok(client_message)) => client_message,
                    Some(Err(e)) => {
                        error!(
                            "Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to read from client because of error: {e:?}"
                        );
                        if let Err(e) = proxy_connection_write.close().await {
                            error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close client connection because of error: {e:?}");
                        };
                        stop_read_client=true;
                        continue;
                    },
                    None=>{
                        if let Err(e) = proxy_connection_write.close().await {
                            error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close client connection because of error: {e:?}");
                        };
                        stop_read_client=true;
                        continue;
                    }
                   };
                   trace!(
                       "Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] read client data:\n{}\n",
                       pretty_hex::pretty_hex(&client_message)
                   );
                   let tcp_data = PpaassMessageGenerator::generate_tcp_data(&user_token, payload_encryption.clone(), src_address.clone(), dst_address.clone(), client_message.to_vec())?;

                   if let Err(e) = proxy_connection_write.send(tcp_data).await {
                       error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to relay client data to proxy because of error: {e:?}");
                       if let Err(e) = proxy_connection_write.close().await {
                            error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close client connection because of error: {e:?}");
                       };
                       stop_read_client=true;
                       continue;
                   };
                },
                proxy_message = proxy_connection_read.next(), if !stop_read_proxy => {
                    let proxy_message = match proxy_message {
                        Some(Ok(proxy_message))=>proxy_message,
                        Some(Err(e))=>{
                            error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to read from proxy because of error: {e:?}");
                            if let Err(e) = client_io_write.close().await {
                                error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close proxy connection because of error: {e:?}");
                            };
                            stop_read_proxy=true;
                            continue;
                        },
                        None=>{
                            if let Err(e) = client_io_write.close().await {
                                error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close proxy connection because of error: {e:?}");
                            };
                            stop_read_proxy=true;
                            continue;
                        }

                    };
                    let PpaassMessageParts {
                        payload,
                        ..
                    } = proxy_message.split();
                    let tcp_data: TcpData = match payload.try_into(){
                        Ok(tcp_data) => tcp_data,
                        Err(e) => {
                            error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to parse tcp data because of error: {e:?}");
                            if let Err(e) = client_io_write.close().await {
                                error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close proxy connection because of error: {e:?}");
                            };
                            stop_read_proxy=true;
                            continue;
                        },
                    };
                    let TcpDataParts{
                        raw_data,
                        ..
                    } = tcp_data.split();
                    trace!(
                        "Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] read proxy data:\n{}\n",
                        pretty_hex::pretty_hex(&raw_data)
                    );
                    if let Err(e) = client_io_write.send(BytesMut::from_iter(raw_data)).await {
                        error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to relay to proxy because of error: {e:?}");
                        if let Err(e) = client_io_write.close().await {
                            error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close proxy connection because of error: {e:?}");
                        };
                        stop_read_proxy=true;
                        continue;
                    };
                }
            }
        }
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
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_ready(cx).map_err(|e| anyhow!(e))
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        this.client_bytes_framed_write.start_send(item).map_err(|e| anyhow::anyhow!(e))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_flush(cx).map_err(|e| anyhow::anyhow!(e))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.client_bytes_framed_write.poll_close(cx).map_err(|e| anyhow::anyhow!(e))
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
    type Item = Result<BytesMut, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.client_bytes_framed_read.poll_next(cx).map_err(|e| anyhow::anyhow!(e))
    }
}
