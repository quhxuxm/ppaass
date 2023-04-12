pub(crate) mod dispatcher;

use std::{
    fmt::{Debug, Display},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use anyhow::Result;

use bytes::BytesMut;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, SinkExt, Stream, StreamExt,
};
use pin_project::{pin_project, pinned_drop};
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
use tracing::{debug, error, trace};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, pool::ProxyConnectionPool};

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

pub(crate) enum ClientProcessor {
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

impl ClientProcessor {
    pub(crate) async fn exec<R, I>(self, proxy_connection_pool: Arc<ProxyConnectionPool>, configuration: Arc<AgentServerConfig>) -> Result<()>
    where
        R: RsaCryptoFetcher + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + 'static,
    {
        match self {
            ClientProcessor::Http {
                client_tcp_stream,
                src_address,
                initial_buf,
            } => {
                let http_flow = HttpClientProcessor::new(client_tcp_stream, src_address.clone());
                if let Err(e) = http_flow
                    .exec::<AgentServerRsaCryptoFetcher, String>(proxy_connection_pool, configuration, initial_buf)
                    .await
                {
                    error!("Client tcp connection [{src_address}] error happen on http flow for proxy connection: {e:?}");
                    return Err(e);
                }
            },
            ClientProcessor::Socks5 {
                client_tcp_stream,
                src_address,
                initial_buf,
            } => {
                let socks5_flow = Socks5ClientProcessor::new(client_tcp_stream, src_address.clone());
                if let Err(e) = socks5_flow
                    .exec::<AgentServerRsaCryptoFetcher, String>(proxy_connection_pool, configuration, initial_buf)
                    .await
                {
                    error!("Client tcp connection [{src_address}] error happen on socks5 flow for proxy connection: {e:?}");
                    return Err(e);
                };
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
        let client_io_framed = Framed::with_capacity(client_tcp_stream, BytesCodec::new(), configuration.get_client_io_buffer_size());
        let (client_io_write, client_io_read) = client_io_framed.split::<BytesMut>();
        let (mut client_io_write, mut client_io_read) = (
            ClientConnectionWrite::new(src_address.clone(), tcp_loop_key.clone(), client_io_write),
            ClientConnectionRead::new(src_address.clone(), tcp_loop_key.clone(), client_io_read),
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
                if let Err(e) = client_io_write.close().await {
                    error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close client connection because of error: {e:?}");
                };
                if let Err(e) = proxy_connection_write.close().await {
                    error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to close proxy connection because of error: {e:?}");
                };
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
                        stop_read_client=true;
                        continue;
                    },
                    None=>{
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
                       error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to relay client data to proxy because of error: {e:?}");stop_read_client=true;
                       continue;
                   };
                },
                proxy_message = proxy_connection_read.next(), if !stop_read_proxy => {
                    let proxy_message = match proxy_message {
                        Some(Err(e))=>{
                            error!("Client tcp connection [{src_address}] for tcp loop [{tcp_loop_key}] fail to read from proxy because of error: {e:?}");
                            stop_read_proxy=true;
                            continue;
                        },
                        Some(Ok(proxy_message))=>proxy_message,
                        None=>{
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
                        stop_read_proxy=true;
                        continue;
                    };
                }
            }
        }
    }
}

#[pin_project(PinnedDrop)]
struct ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    src_address: PpaassNetAddress,
    tcp_loop_key: String,
    #[pin]
    client_bytes_framed_write: Option<SplitSink<Framed<T, BytesCodec>, BytesMut>>,
}

impl<T> ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn new(src_address: PpaassNetAddress, tcp_loop_key: String, client_bytes_framed_write: SplitSink<Framed<T, BytesCodec>, BytesMut>) -> Self {
        Self {
            src_address,
            tcp_loop_key,
            client_bytes_framed_write: Some(client_bytes_framed_write),
        }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let src_address = this.src_address.clone();
        let tcp_loop_key = this.tcp_loop_key.clone();
        if let Some(mut client_bytes_framed_write) = this.client_bytes_framed_write.take() {
            tokio::spawn(async move {
                debug!("Client connection {src_address} with tcp loop key {tcp_loop_key} drop dest connection write.");
                if let Err(e) = client_bytes_framed_write.close().await {
                    error!("Client connection {src_address} with tcp loop key {tcp_loop_key}, error happen on drop dest connection write: {e:?}")
                }
            });
        };
    }
}

impl<T> Sink<BytesMut> for ClientConnectionWrite<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(client_bytes_framed_write) = this.client_bytes_framed_write.as_pin_mut() {
            match client_bytes_framed_write.poll_ready(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow::anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow::anyhow!("Client bytes framed write not exist")))
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        if let Some(client_bytes_framed_write) = this.client_bytes_framed_write.as_pin_mut() {
            return client_bytes_framed_write.start_send(item).map_err(|e| anyhow::anyhow!(e));
        }
        Err(anyhow::anyhow!("Client bytes framed write not exist"))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(client_bytes_framed_write) = this.client_bytes_framed_write.as_pin_mut() {
            match client_bytes_framed_write.poll_flush(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow::anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow::anyhow!("Client bytes framed write not exist")))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(client_bytes_framed_write) = this.client_bytes_framed_write.as_pin_mut() {
            match client_bytes_framed_write.poll_close(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow::anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow::anyhow!("Client bytes framed write not exist")))
    }
}

#[pin_project(PinnedDrop)]
struct ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    src_address: PpaassNetAddress,
    tcp_loop_key: String,
    #[pin]
    client_bytes_framed_read: Option<SplitStream<Framed<T, BytesCodec>>>,
}

impl<T> ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn new(src_address: PpaassNetAddress, tcp_loop_key: String, client_bytes_framed_read: SplitStream<Framed<T, BytesCodec>>) -> Self {
        Self {
            src_address,
            tcp_loop_key,
            client_bytes_framed_read: Some(client_bytes_framed_read),
        }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let src_address = this.src_address.clone();
        let tcp_loop_key = this.tcp_loop_key.clone();
        if let Some(client_bytes_framed_read) = this.client_bytes_framed_read.take() {
            tokio::spawn(async move {
                debug!("Client connection {src_address} with tcp loop key {tcp_loop_key} drop dest connection read.");
                drop(client_bytes_framed_read)
            });
        };
    }
}

impl<T> Stream for ClientConnectionRead<T>
where
    T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    type Item = Result<BytesMut, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if let Some(client_bytes_framed_read) = this.client_bytes_framed_read.as_pin_mut() {
            return match client_bytes_framed_read.poll_next(cx) {
                Poll::Ready(value) => match value {
                    None => Poll::Ready(None),
                    Some(value) => Poll::Ready(Some(value.map_err(|e| anyhow::anyhow!(e)))),
                },
                Poll::Pending => Poll::Pending,
            };
        }
        Poll::Ready(None)
    }
}
