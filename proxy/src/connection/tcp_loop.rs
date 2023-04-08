use std::{fmt::Debug, task::Poll};
use std::{fmt::Display, pin::Pin};
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use bytes::BytesMut;
use futures::{
    stream::{SplitSink, SplitStream},
    Sink, StreamExt,
};
use futures_util::{try_join, SinkExt, Stream};
use pin_project::{pin_project, pinned_drop};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio::{task::JoinHandle, time::timeout};
use tokio_util::codec::{BytesCodec, Framed};
use tracing::{debug, error, trace};

use ppaass_common::{generate_uuid, PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageParts, RsaCryptoFetcher};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig};

type DestBytesFramedRead = SplitStream<Framed<TcpStream, BytesCodec>>;
type DestBytesFramedWrite = SplitSink<Framed<TcpStream, BytesCodec>, BytesMut>;

pub(crate) struct TcpLoopBuilder<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    agent_connection_id: Option<String>,
    agent_connection_read: Option<PpaassConnectionRead<T, R, I>>,
    agent_connection_write: Option<PpaassConnectionWrite<T, R, I>>,
    user_token: Option<String>,
    agent_address: Option<PpaassNetAddress>,
    src_address: Option<PpaassNetAddress>,
    dest_address: Option<PpaassNetAddress>,
}

impl<T, R, I> TcpLoopBuilder<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            agent_connection_id: None,
            agent_connection_read: None,
            agent_connection_write: None,
            user_token: None,
            agent_address: None,
            src_address: None,
            dest_address: None,
        }
    }
    pub(crate) fn agent_connection_id(mut self, agent_connection_id: impl AsRef<str>) -> TcpLoopBuilder<T, R, I> {
        self.agent_connection_id = Some(agent_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> TcpLoopBuilder<T, R, I> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> TcpLoopBuilder<T, R, I> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn src_address(mut self, src_address: PpaassNetAddress) -> TcpLoopBuilder<T, R, I> {
        self.src_address = Some(src_address);
        self
    }

    pub(crate) fn dest_address(mut self, dest_address: PpaassNetAddress) -> TcpLoopBuilder<T, R, I> {
        self.dest_address = Some(dest_address);
        self
    }

    pub(crate) fn agent_connection_read(mut self, agent_connection_read: PpaassConnectionRead<T, R, I>) -> TcpLoopBuilder<T, R, I> {
        self.agent_connection_read = Some(agent_connection_read);
        self
    }

    pub(crate) fn agent_connection_write(mut self, agent_connection_write: PpaassConnectionWrite<T, R, I>) -> TcpLoopBuilder<T, R, I> {
        self.agent_connection_write = Some(agent_connection_write);
        self
    }

    pub(crate) async fn build(self, configuration: Arc<ProxyServerConfig>) -> Result<TcpLoop<T, R, I>> {
        let agent_connection_id = self.agent_connection_id.context("Agent connection id not assigned for tcp loop builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for tcp loop builder")?;
        let src_address = self.src_address.context("Source address not assigned for tcp loop builder")?;
        let dest_address = self.dest_address.context("Destination address not assigned for tcp loop builder")?;
        let user_token = self.user_token.context("User token not assigned for tcp loop builder")?;
        let key = TcpLoop::<T, R, I>::generate_key(&agent_address, &src_address, &dest_address);
        let mut agent_connection_write = self
            .agent_connection_write
            .context("Agent message framed write not assigned for tcp loop builder")?;
        let agent_connection_read = self
            .agent_connection_read
            .context("Agent message framed read not assigned for tcp loop builder")?;
        let dest_socket_address = match dest_address.to_socket_addrs() {
            Ok(v) => v,
            Err(e) => {
                error!("Fail to convert dest address [{dest_address}] to valid socket address because of error:{e:?}");
                return Err(anyhow::anyhow!(e));
            },
        };
        let dest_socket_address = dest_socket_address.collect::<Vec<SocketAddr>>();
        let dest_tcp = match timeout(
            Duration::from_secs(configuration.get_dest_connect_timeout()),
            TcpStream::connect(dest_socket_address.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Agent connection [{agent_connection_id}] fail connect to dest address [{dest_address}] because of timeout.");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_initialize_fail_message =
                    PpaassMessageGenerator::generate_tcp_loop_init_fail_response(&key, &user_token, src_address, dest_address, payload_encryption_token)?;
                agent_connection_write.send(tcp_initialize_fail_message).await.context(format!(
                    "Agent connection [{agent_connection_id}] fail to send tcp loop init fail response to agent."
                ))?;
                return Err(anyhow::anyhow!(
                    "Agent connection [{agent_connection_id}] fail connect to dest address because of timeout."
                ));
            },
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                error!("Agent connection [{agent_connection_id}] fail connect to dest address [{dest_address}] because of error: {e:?}");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_initialize_fail_message =
                    PpaassMessageGenerator::generate_tcp_loop_init_fail_response(&key, &user_token, src_address, dest_address, payload_encryption_token)?;
                agent_connection_write.send(tcp_initialize_fail_message).await.context(format!(
                    "Agent connection [{agent_connection_id}] fail to send tcp loop init fail response to agent."
                ))?;
                return Err(anyhow::anyhow!(e));
            },
        };

        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_initialize_success_message = PpaassMessageGenerator::generate_tcp_loop_init_success_response(
            &key,
            &user_token,
            src_address.clone(),
            dest_address.clone(),
            payload_encryption_token,
        )?;
        if let Err(e) = agent_connection_write.send(tcp_initialize_success_message).await {
            error!("Agent connection [{agent_connection_id}] fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow::anyhow!(e));
        };
        Ok(TcpLoop {
            key,
            agent_connection_read,
            agent_connection_write,
            dest_tcp,
            user_token,
            agent_connection_id,
            configuration,
        })
    }
}

#[pin_project(PinnedDrop)]
struct DestConnectionWrite {
    agent_connection_id: String,
    tcp_loop_key: String,
    #[pin]
    dest_bytes_framed_write: Option<DestBytesFramedWrite>,
}

impl DestConnectionWrite {
    fn new(agent_connection_id: String, tcp_loop_key: String, dest_bytes_framed_write: DestBytesFramedWrite) -> Self {
        Self {
            agent_connection_id,
            tcp_loop_key,
            dest_bytes_framed_write: Some(dest_bytes_framed_write),
        }
    }
}

#[pinned_drop]
impl PinnedDrop for DestConnectionWrite {
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let agent_connection_id = this.agent_connection_id.clone();
        let tcp_loop_key = this.tcp_loop_key.clone();
        if let Some(mut dest_bytes_framed_write) = this.dest_bytes_framed_write.take() {
            tokio::spawn(async move {
                debug!("Agent connection {agent_connection_id} with tcp loop key {tcp_loop_key} drop dest connection write.");
                if let Err(e) = dest_bytes_framed_write.close().await {
                    error!("Agent connection {agent_connection_id} with tcp loop key {tcp_loop_key}, error happen on drop dest connection write: {e:?}")
                }
            });
        };
    }
}

impl Sink<BytesMut> for DestConnectionWrite {
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            match dest_bytes_framed_write.poll_ready(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow::anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow::anyhow!("Dest bytes framed write not exist")))
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            return dest_bytes_framed_write.start_send(item).map_err(|e| anyhow::anyhow!(e));
        }
        Err(anyhow::anyhow!("Dest bytes framed write not exist"))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            match dest_bytes_framed_write.poll_flush(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow::anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow::anyhow!("Dest bytes framed write not exist")))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        if let Some(dest_bytes_framed_write) = this.dest_bytes_framed_write.as_pin_mut() {
            match dest_bytes_framed_write.poll_close(cx) {
                Poll::Ready(value) => return Poll::Ready(value.map_err(|e| anyhow::anyhow!(e))),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Err(anyhow::anyhow!("Dest bytes framed write not exist")))
    }
}

#[pin_project(PinnedDrop)]
struct DestConnectionRead {
    agent_connection_id: String,
    tcp_loop_key: String,
    #[pin]
    dest_bytes_framed_read: Option<DestBytesFramedRead>,
}

impl DestConnectionRead {
    fn new(agent_connection_id: String, tcp_loop_key: String, dest_bytes_framed_read: DestBytesFramedRead) -> Self {
        Self {
            agent_connection_id,
            tcp_loop_key,
            dest_bytes_framed_read: Some(dest_bytes_framed_read),
        }
    }
}

#[pinned_drop]
impl PinnedDrop for DestConnectionRead {
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let agent_connection_id = this.agent_connection_id.clone();
        let tcp_loop_key = this.tcp_loop_key.clone();
        if let Some(dest_bytes_framed_read) = this.dest_bytes_framed_read.take() {
            tokio::spawn(async move {
                debug!("Agent connection {agent_connection_id} with tcp loop key {tcp_loop_key} drop dest connection read.");
                drop(dest_bytes_framed_read)
            });
        };
    }
}

impl Stream for DestConnectionRead {
    type Item = Result<BytesMut, anyhow::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        if let Some(dest_bytes_framed_read) = this.dest_bytes_framed_read.as_pin_mut() {
            return match dest_bytes_framed_read.poll_next(cx) {
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

#[derive(Debug)]
pub(crate) struct TcpLoop<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    dest_tcp: TcpStream,
    agent_connection_read: PpaassConnectionRead<T, R, I>,
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    key: String,
    user_token: String,
    agent_connection_id: String,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R, I> TcpLoop<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn generate_key(agent_address: &PpaassNetAddress, src_address: &PpaassNetAddress, dest_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]::[{src_address}=>{dest_address}]")
    }

    fn start_dest_to_agent_relay(
        agent_connection_id: impl AsRef<str>, tcp_loop_key: impl AsRef<str>, mut agent_connection_write: PpaassConnectionWrite<T, R, I>,
        mut dest_tcp_read: DestConnectionRead, user_token: impl AsRef<str>,
    ) -> JoinHandle<Result<()>> {
        let user_token = user_token.as_ref().to_owned();
        let key = tcp_loop_key.as_ref().to_owned();
        let agent_connection_id = agent_connection_id.as_ref().to_owned();
        tokio::spawn(async move {
            debug!("Agent connection [{agent_connection_id}] with tcp loop [{key}] start to relay destination data to agent.");
            let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
            loop {
                let dest_message = match dest_tcp_read.next().await {
                    None => {
                        debug!("Agent connection [{agent_connection_id}] with tcp loop [{key}] complete to relay destination data to agent.");
                        break;
                    },
                    Some(Ok(dest_message)) => dest_message,
                    Some(Err(e)) => {
                        error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to read destination data because of error: {e:?}");
                        return Err(anyhow::anyhow!(e));
                    },
                };
                trace!(
                    "Agent connection [{agent_connection_id}] with tcp loop [{key}] read destination data:\n{}\n",
                    pretty_hex::pretty_hex(&dest_message)
                );
                let tcp_relay = match PpaassMessageGenerator::generate_tcp_raw_data(&user_token, payload_encryption_token.clone(), dest_message.to_vec()) {
                    Ok(tcp_relay) => tcp_relay,
                    Err(e) => {
                        error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to generate raw data because of error: {e:?}");
                        return Err(anyhow::anyhow!(e));
                    },
                };
                if let Err(e) = agent_connection_write.send(tcp_relay).await {
                    error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to relay destination data to agent because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }

            Ok(())
        })
    }

    fn start_agent_to_dest_relay(
        agent_connection_id: impl AsRef<str>, tcp_loop_key: impl AsRef<str>, mut agent_connection_read: PpaassConnectionRead<T, R, I>,
        mut dest_tcp_write: DestConnectionWrite,
    ) -> JoinHandle<Result<()>> {
        let key = tcp_loop_key.as_ref().to_owned();
        let agent_connection_id = agent_connection_id.as_ref().to_owned();
        debug!("Agent connection [{agent_connection_id}] with tcp loop [{key}] start to relay agent data to destination.");
        tokio::spawn(async move {
            while let Some(agent_message) = agent_connection_read.next().await {
                let agent_message = match agent_message {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to read agent message because of error: {e:?}");
                        return Err(anyhow::anyhow!(e));
                    },
                };
                let PpaassMessageParts { payload_bytes, .. } = agent_message.split();
                trace!(
                    "Agent connection [{agent_connection_id}] with tcp loop [{key}] read agent data:\n{}\n",
                    pretty_hex::pretty_hex(&payload_bytes)
                );
                let payload_bytes = BytesMut::from_iter(payload_bytes);
                if let Err(e) = dest_tcp_write.send(payload_bytes).await {
                    error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to relay agent message to destination because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }
            debug!("Agent connection [{agent_connection_id}] with tcp loop [{key}] complete to relay agent data to destination.");
            Ok(())
        })
    }

    pub(crate) fn get_key(&self) -> &str {
        self.key.as_str()
    }

    pub(crate) async fn exec(self) -> Result<()> {
        let agent_connection_id = self.agent_connection_id.clone();
        let tcp_loop_key = self.key.clone();
        let dest_tcp = self.dest_tcp;
        let dest_bytes_framed = Framed::with_capacity(dest_tcp, BytesCodec::new(), self.configuration.get_dest_tcp_buffer_size());
        let (dest_tcp_write, dest_tcp_read) = dest_bytes_framed.split::<BytesMut>();
        let (dest_tcp_write, dest_tcp_read) = (
            DestConnectionWrite::new(agent_connection_id.clone(), tcp_loop_key.clone(), dest_tcp_write),
            DestConnectionRead::new(agent_connection_id, tcp_loop_key, dest_tcp_read),
        );
        let agent_connection_write = self.agent_connection_write;
        let agent_connection_read = self.agent_connection_read;
        let user_token = self.user_token;
        let key = self.key;
        let agent_connection_id = self.agent_connection_id;
        let mut dest_to_agent_relay_guard =
            Self::start_dest_to_agent_relay(agent_connection_id.clone(), key.clone(), agent_connection_write, dest_tcp_read, &user_token);
        let mut agent_to_dest_relay_guard = Self::start_agent_to_dest_relay(agent_connection_id.clone(), key.clone(), agent_connection_read, dest_tcp_write);
        if let Err(e) = try_join!(&mut dest_to_agent_relay_guard, &mut agent_to_dest_relay_guard) {
            dest_to_agent_relay_guard.abort();
            agent_to_dest_relay_guard.abort();
            error!("Agent connection [{agent_connection_id}] for tcp loop [{key}] fail to do relay process because of error: {e:?}");
            return Err(anyhow::anyhow!(e));
        };
        Ok(())
    }
}
