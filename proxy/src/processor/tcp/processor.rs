use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context as AnyhowContext, Result};
use bytes::BytesMut;
use futures::StreamExt;
use futures_util::SinkExt;

use tokio::time::timeout;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_util::codec::{BytesCodec, Framed};
use tracing::{debug, error, trace};

use ppaass_common::{generate_uuid, PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageParts, RsaCryptoFetcher};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig};

use super::destination::{DestConnectionRead, DestConnectionWrite};

pub(crate) struct TcpProcessorBuilder<T, R, I>
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

impl<T, R, I> TcpProcessorBuilder<T, R, I>
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
    pub(crate) fn agent_connection_id(mut self, agent_connection_id: impl AsRef<str>) -> TcpProcessorBuilder<T, R, I> {
        self.agent_connection_id = Some(agent_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> TcpProcessorBuilder<T, R, I> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> TcpProcessorBuilder<T, R, I> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn src_address(mut self, src_address: PpaassNetAddress) -> TcpProcessorBuilder<T, R, I> {
        self.src_address = Some(src_address);
        self
    }

    pub(crate) fn dest_address(mut self, dest_address: PpaassNetAddress) -> TcpProcessorBuilder<T, R, I> {
        self.dest_address = Some(dest_address);
        self
    }

    pub(crate) fn agent_connection_read(mut self, agent_connection_read: PpaassConnectionRead<T, R, I>) -> TcpProcessorBuilder<T, R, I> {
        self.agent_connection_read = Some(agent_connection_read);
        self
    }

    pub(crate) fn agent_connection_write(mut self, agent_connection_write: PpaassConnectionWrite<T, R, I>) -> TcpProcessorBuilder<T, R, I> {
        self.agent_connection_write = Some(agent_connection_write);
        self
    }

    pub(crate) async fn build(self, configuration: Arc<ProxyServerConfig>) -> Result<TcpProcessor<T, R, I>> {
        let agent_connection_id = self.agent_connection_id.context("Agent connection id not assigned for tcp loop builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for tcp loop builder")?;
        let src_address = self.src_address.context("Source address not assigned for tcp loop builder")?;
        let dest_address = self.dest_address.context("Destination address not assigned for tcp loop builder")?;
        let user_token = self.user_token.context("User token not assigned for tcp loop builder")?;
        let key = TcpProcessor::<T, R, I>::generate_key(&agent_address, &src_address, &dest_address);
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
                return Err(anyhow!(e));
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
                return Err(anyhow!(
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
                return Err(anyhow!(e));
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
            return Err(anyhow!(e));
        };
        Ok(TcpProcessor {
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

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct TcpProcessor<T, R, I>
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

impl<T, R, I> TcpProcessor<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn generate_key(agent_address: &PpaassNetAddress, src_address: &PpaassNetAddress, dest_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]::[{src_address}=>{dest_address}]")
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
        let (mut dest_tcp_write, mut dest_tcp_read) = (
            DestConnectionWrite::new(agent_connection_id.clone(), tcp_loop_key.clone(), dest_tcp_write),
            DestConnectionRead::new(agent_connection_id, tcp_loop_key, dest_tcp_read),
        );
        let mut agent_connection_write = self.agent_connection_write;
        let mut agent_connection_read = self.agent_connection_read;
        let user_token = self.user_token;
        let key = self.key;
        let agent_connection_id = self.agent_connection_id;
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let mut stop_read_agent = false;
        let mut stop_read_dst = false;
        loop {
            if stop_read_agent && stop_read_dst {
                if let Err(e) = agent_connection_write.close().await {
                    error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to close agent connection because of error: {e:?}");
                };
                if let Err(e) = dest_tcp_write.close().await {
                    error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to close destination connection because of error: {e:?}");
                };
                break Ok(());
            }
            tokio::select! {
                agent_message = agent_connection_read.next(), if !stop_read_agent => {
                    let agent_message = match agent_message {
                        Some(Ok(agent_message)) => agent_message,
                        Some(Err(e))=>{
                            error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to read agent message because of error: {e:?}");
                            stop_read_agent = true;
                            continue;
                        }
                        None => {
                            debug!("Agent connection [{agent_connection_id}] with tcp loop [{key}] complete to read agent message.");
                            stop_read_agent = true;
                            continue;
                        },
                    };
                    let PpaassMessageParts { payload_bytes, .. } = agent_message.split();
                    trace!("Agent connection [{agent_connection_id}] with tcp loop [{key}] read agent data:\n{}\n", pretty_hex::pretty_hex(&payload_bytes));
                    let payload_bytes = BytesMut::from_iter(payload_bytes);
                    if let Err(e) = dest_tcp_write.send(payload_bytes).await {
                        error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to relay agent message to destination because of error: {e:?}");
                        stop_read_agent = true;
                        continue;
                    };
                },
                dst_message = dest_tcp_read.next(), if !stop_read_dst => {
                    let dst_message = match dst_message {
                        None => {
                            debug!("Agent connection [{agent_connection_id}] with tcp loop [{key}] complete to read destination data.");
                            stop_read_dst=true;
                            continue;
                        },
                        Some(Ok(dest_message)) => dest_message,
                        Some(Err(e)) => {
                            error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to read destination data because of error: {e:?}");
                            stop_read_dst=true;
                            continue;
                        },
                    };
                    trace!("Agent connection [{agent_connection_id}] with tcp loop [{key}] read destination data:\n{}\n", pretty_hex::pretty_hex(&dst_message));
                    let tcp_relay = match PpaassMessageGenerator::generate_tcp_raw_data(&user_token, payload_encryption_token.clone(), dst_message.to_vec()) {
                        Ok(tcp_relay) => tcp_relay,
                        Err(e) => {
                            error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to generate raw data because of error: {e:?}");
                            stop_read_dst=true;
                            continue;
                        },
                    };
                    if let Err(e) = agent_connection_write.send(tcp_relay).await {
                        error!("Agent connection [{agent_connection_id}] with tcp loop [{key}] fail to relay destination data to agent because of error: {e:?}");
                        stop_read_dst=true;
                        continue;
                    };
                }
            }
        }
    }
}
