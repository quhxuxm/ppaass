use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig};
use anyhow::{Context, Result};

use bytes::BytesMut;
use futures::{
    stream::{SplitSink, SplitStream},
    StreamExt,
};

use futures_util::SinkExt;
use ppaass_common::{generate_uuid, PpaassMessageParts, RsaCryptoFetcher};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};
use tokio_util::codec::{BytesCodec, Framed};

use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};
use tokio::{task::JoinHandle, time::timeout};

use tracing::{debug, error};

use super::{AgentMessageFramedRead, AgentMessageFramedWrite};

pub(crate) struct TcpLoopBuilder<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    agent_connection_id: Option<String>,
    agent_message_framed_read: Option<AgentMessageFramedRead<T, R>>,
    agent_message_framed_write: Option<AgentMessageFramedWrite<T, R>>,
    user_token: Option<String>,
    agent_address: Option<PpaassNetAddress>,
    src_address: Option<PpaassNetAddress>,
    dest_address: Option<PpaassNetAddress>,
}

impl<T, R> TcpLoopBuilder<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            agent_connection_id: None,
            agent_message_framed_read: None,
            agent_message_framed_write: None,
            user_token: None,
            agent_address: None,
            src_address: None,
            dest_address: None,
        }
    }
    pub(crate) fn agent_connection_id(mut self, agent_connection_id: impl AsRef<str>) -> TcpLoopBuilder<T, R> {
        self.agent_connection_id = Some(agent_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> TcpLoopBuilder<T, R> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> TcpLoopBuilder<T, R> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn src_address(mut self, src_address: PpaassNetAddress) -> TcpLoopBuilder<T, R> {
        self.src_address = Some(src_address);
        self
    }

    pub(crate) fn dest_address(mut self, dest_address: PpaassNetAddress) -> TcpLoopBuilder<T, R> {
        self.dest_address = Some(dest_address);
        self
    }

    pub(crate) fn agent_message_framed_read(mut self, agent_message_framed_read: AgentMessageFramedRead<T, R>) -> TcpLoopBuilder<T, R> {
        self.agent_message_framed_read = Some(agent_message_framed_read);
        self
    }

    pub(crate) fn agent_message_framed_write(mut self, agent_message_framed_write: AgentMessageFramedWrite<T, R>) -> TcpLoopBuilder<T, R> {
        self.agent_message_framed_write = Some(agent_message_framed_write);
        self
    }

    pub(crate) async fn build(self, configuration: Arc<ProxyServerConfig>) -> Result<TcpLoop<T, R>> {
        let agent_connection_id = self.agent_connection_id.context("Agent connection id not assigned for tcp loop builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for tcp loop builder")?;
        let src_address = self.src_address.context("Source address not assigned for tcp loop builder")?;
        let dest_address = self.dest_address.context("Destination address not assigned for tcp loop builder")?;
        let user_token = self.user_token.context("User token not assigned for tcp loop builder")?;
        let key = TcpLoop::<T, R>::generate_key(&agent_address, &src_address, &dest_address);
        let mut agent_message_framed_write = self
            .agent_message_framed_write
            .context("Agent message framed write not assigned for tcp loop builder")?;
        let agent_message_framed_read = self
            .agent_message_framed_read
            .context("Agent message framed read not assigned for tcp loop builder")?;
        let dest_socket_address = dest_address.to_socket_addrs().context("Convert destination address to socket address")?;
        let dest_socket_address = dest_socket_address.collect::<Vec<SocketAddr>>();
        let dest_tcp_stream = match timeout(
            Duration::from_secs(configuration.get_dest_connect_timeout()),
            TcpStream::connect(dest_socket_address.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Agent connection [{agent_connection_id}] fail connect to dest address because of timeout.");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_initialize_fail_message =
                    PpaassMessageGenerator::generate_tcp_loop_init_fail_response(&key, &user_token, src_address, dest_address, payload_encryption_token)?;
                agent_message_framed_write
                    .send(tcp_initialize_fail_message)
                    .await
                    .context("Agent connection [{agent_connection_id}] fail to send tcp initialize fail message to agent")?;
                return Err(anyhow::anyhow!(
                    "Agent connection [{agent_connection_id}] fail connect to dest address because of timeout."
                ));
            },
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                error!("Agent connection [{agent_connection_id}] fail connect to dest address because of error: {e:?}");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_initialize_fail_message =
                    PpaassMessageGenerator::generate_tcp_loop_init_fail_response(&key, &user_token, src_address, dest_address, payload_encryption_token)?;
                agent_message_framed_write
                    .send(tcp_initialize_fail_message)
                    .await
                    .context("Agent connection [{agent_connection_id}] fail to send tcp initialize fail message to agent")?;
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
        if let Err(e) = agent_message_framed_write.send(tcp_initialize_success_message).await {
            error!("Agent connection [{agent_connection_id}] fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow::anyhow!(e));
        };
        if let Err(e) = agent_message_framed_write.flush().await {
            error!("Agent connection [{agent_connection_id}] fail to send tcp initialize success message to agent(flush) because of error: {e:?}");
            return Err(anyhow::anyhow!(e));
        };

        Ok(TcpLoop {
            key,
            agent_message_framed_read,
            agent_message_framed_write,
            dest_io: dest_tcp_stream,
            user_token,
            agent_connection_id,
            configuration,
        })
    }
}

#[derive(Debug)]
pub(crate) struct TcpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    dest_io: TcpStream,
    agent_message_framed_read: AgentMessageFramedRead<T, R>,
    agent_message_framed_write: AgentMessageFramedWrite<T, R>,
    key: String,
    user_token: String,
    agent_connection_id: String,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R> TcpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    fn generate_key(agent_address: &PpaassNetAddress, src_address: &PpaassNetAddress, dest_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]::[{src_address}=>{dest_address}]")
    }
    fn start_dest_to_agent_relay(
        agent_connection_id: impl AsRef<str>, tcp_loop_key: impl AsRef<str>, mut agent_message_framed_write: AgentMessageFramedWrite<T, R>,
        mut dest_io_read: OwnedReadHalf, user_token: impl AsRef<str>, configuration: Arc<ProxyServerConfig>,
    ) -> JoinHandle<Result<()>> {
        let user_token = user_token.as_ref().to_owned();
        let key = tcp_loop_key.as_ref().to_owned();
        let agent_connection_id = agent_connection_id.as_ref().to_owned();
        tokio::spawn(async move {
            debug!("Agent connection [{agent_connection_id}] tcp loop [{key}] start to relay destination data to agent.");
            let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
            loop {
                let mut dest_message = Vec::with_capacity(configuration.get_dest_io_buffer_size());
                match timeout(
                    Duration::from_secs(configuration.get_dest_read_timeout()),
                    dest_io_read.read_buf(&mut dest_message),
                )
                .await
                {
                    Err(_) => {
                        error!("Agent connection [{agent_connection_id}] tcp loop [{key}] fail to read destination data because of timeout");
                        return Err(anyhow::anyhow!(
                            "Agent connection [{agent_connection_id}] tcp loop [{key}] fail to read destination data because of timeout"
                        ));
                    },
                    Ok(Ok(0)) => {
                        debug!("Agent connection [{agent_connection_id}] tcp loop [{key}] complete relay destination data to agent.");
                        break;
                    },
                    Ok(Ok(dest_message)) => dest_message,
                    Ok(Err(e)) => {
                        error!("Agent connection [{agent_connection_id}] tcp loop [{key}] fail to read destination data because of error: {e:?}");
                        return Err(anyhow::anyhow!(e));
                    },
                };
                debug!(
                    "Agent connection [{agent_connection_id}] tcp loop [{key}] read destination data:\n{}\n",
                    pretty_hex::pretty_hex(&dest_message)
                );
                let tcp_relay = PpaassMessageGenerator::generate_raw_data(&user_token, payload_encryption_token.clone(), dest_message.to_vec())?;
                if let Err(e) = agent_message_framed_write.send(tcp_relay).await {
                    error!("Agent connection [{agent_connection_id}] tcp loop [{key}] fail to relay destination data to agent because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }

            Ok(())
        })
    }

    fn start_agent_to_dest_relay(
        agent_connection_id: impl AsRef<str>, tcp_loop_key: impl AsRef<str>, mut agent_message_framed_read: AgentMessageFramedRead<T, R>,
        mut dest_io_write: OwnedWriteHalf,
    ) -> JoinHandle<Result<()>> {
        let key = tcp_loop_key.as_ref().to_owned();
        let agent_connection_id = agent_connection_id.as_ref().to_owned();
        debug!("Agent connection [{agent_connection_id}] tcp loop [{key}] start to relay agent data to destination.");
        tokio::spawn(async move {
            while let Some(agent_message) = agent_message_framed_read.next().await {
                let agent_message = match agent_message {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Agent connection [{agent_connection_id}] tcp loop [{key}] fail to read agent message becuase of error: {e:?}");
                        return Err(e);
                    },
                };
                let PpaassMessageParts { payload_bytes, .. } = agent_message.split();
                debug!(
                    "Agent connection [{agent_connection_id}] tcp loop [{key}] read agent data:\n{}\n",
                    pretty_hex::pretty_hex(&payload_bytes)
                );
                let payload_bytes = BytesMut::from_iter(payload_bytes);
                if let Err(e) = dest_io_write.write_all(&payload_bytes).await {
                    error!("Agent connection [{agent_connection_id}] tcp loop [{key}] fail to relay agent message to destination becuase of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
                if let Err(e) = dest_io_write.flush().await {
                    error!(
                        "Agent connection [{agent_connection_id}] tcp loop [{key}] fail to relay agent message to destination becuase of error(flush): {e:?}"
                    );
                    return Err(anyhow::anyhow!(e));
                };
            }
            debug!("Agent connection [{agent_connection_id}] tcp loop [{key}] complete relay agent data to destination.");
            Ok(())
        })
    }

    pub(crate) fn get_key(&self) -> &str {
        self.key.as_str()
    }

    pub(crate) async fn start(self) -> Result<()> {
        let dest_io = self.dest_io;
        let (dest_io_read, dest_io_write) = dest_io.into_split();
        let agent_message_framed_write = self.agent_message_framed_write;
        let agent_message_framed_read = self.agent_message_framed_read;
        let user_token = self.user_token;
        let key = self.key;
        let agent_connection_id = self.agent_connection_id;
        let mut dest_to_agent_relay_guard = Self::start_dest_to_agent_relay(
            agent_connection_id.clone(),
            key.clone(),
            agent_message_framed_write,
            dest_io_read,
            &user_token,
            self.configuration.clone(),
        );
        let mut agent_to_dest_relay_guard = Self::start_agent_to_dest_relay(agent_connection_id.clone(), key.clone(), agent_message_framed_read, dest_io_write);
        if let Err(e) = tokio::try_join!(&mut dest_to_agent_relay_guard, &mut agent_to_dest_relay_guard) {
            dest_to_agent_relay_guard.abort();
            agent_to_dest_relay_guard.abort();
            error!("Agent connection [{agent_connection_id}] for tcp loop [{key}] fail to do relay process becuase of error: {e:?}")
        };
        Ok(())
    }
}
