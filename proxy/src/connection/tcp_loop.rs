use crate::common::ProxyServerPayloadEncryptionSelector;
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

use std::net::{SocketAddr, ToSocketAddrs};

use tokio::task::JoinHandle;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use tracing::{debug, error};

use super::{AgentMessageFramedRead, AgentMessageFramedWrite};

#[derive(Debug)]
pub(crate) struct TcpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send,
{
    dest_tcp_stream: TcpStream,
    agent_message_framed_read: AgentMessageFramedRead<T, R>,
    agent_message_framed_write: AgentMessageFramedWrite<T, R>,
    key: String,
    user_token: String,
    agent_connection_id: String,
}

impl<T, R> TcpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    fn generate_key(agent_address: &PpaassNetAddress, src_address: &PpaassNetAddress, dest_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]::[{src_address}=>{dest_address}]")
    }
    pub(crate) async fn new(
        agent_connection_id: impl AsRef<str>, agent_message_framed_read: AgentMessageFramedRead<T, R>,
        mut agent_message_framed_write: AgentMessageFramedWrite<T, R>, user_token: impl AsRef<str>, agent_address: PpaassNetAddress,
        src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
    ) -> Result<Self> {
        let agent_connection_id = agent_connection_id.as_ref().to_owned();
        let key = Self::generate_key(&agent_address, &src_address, &dest_address);
        let user_token = user_token.as_ref().to_owned();
        let socket_address = dest_address.to_socket_addrs().context("Convert destination address to socket address")?;
        let socket_address = socket_address.collect::<Vec<SocketAddr>>();
        let dest_tcp_stream = match TcpStream::connect(socket_address.as_slice()).await {
            Ok(stream) => stream,
            Err(e) => {
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

        Ok(Self {
            key,
            agent_message_framed_read,
            agent_message_framed_write,
            dest_tcp_stream,
            user_token,
            agent_connection_id,
        })
    }

    fn start_dest_to_agent_task(
        agent_connection_id: impl AsRef<str>, tcp_loop_key: impl AsRef<str>, mut agent_message_framed_write: AgentMessageFramedWrite<T, R>,
        mut dest_tcp_framed_read: SplitStream<Framed<TcpStream, BytesCodec>>, user_token: impl AsRef<str>,
    ) -> JoinHandle<Result<()>> {
        let user_token = user_token.as_ref().to_owned();
        let key = tcp_loop_key.as_ref().to_owned();
        let agent_connection_id = agent_connection_id.as_ref().to_owned();
        tokio::spawn(async move {
            debug!("Agent connection [{agent_connection_id}] tcp loop [{key}] start to relay destination data to agent.");
            let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
            while let Some(dest_message) = dest_tcp_framed_read.next().await {
                let dest_message = match dest_message {
                    Ok(v) => v,
                    Err(e) => {
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
                if let Err(e) = agent_message_framed_write.flush().await {
                    error!("Agent connection [{agent_connection_id}] tcp loop [{key}] fail to relay destination data to agent(flush) because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
            }
            debug!("Agent connection [{agent_connection_id}] tcp loop [{key}] read destination data complete.");
            Ok(())
        })
    }

    fn start_agent_to_dest_task(
        agent_connection_id: impl AsRef<str>, tcp_loop_key: impl AsRef<str>, mut agent_message_framed_read: AgentMessageFramedRead<T, R>,
        mut dest_tcp_framed_write: SplitSink<Framed<TcpStream, BytesCodec>, BytesMut>,
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
                if let Err(e) = dest_tcp_framed_write.send(payload_bytes).await {
                    error!("Agent connection [{agent_connection_id}] tcp loop [{key}] fail to relay agent message to destination becuase of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
                };
                if let Err(e) = dest_tcp_framed_write.flush().await {
                    error!(
                        "Agent connection [{agent_connection_id}] tcp loop [{key}] fail to relay agent message to destination(flush) becuase of error: {e:?}"
                    );
                    return Err(anyhow::anyhow!(e));
                };
            }
            debug!("Agent connection [{agent_connection_id}] tcp loop [{key}] complete read agent data.");
            Ok(())
        })
    }

    pub(crate) fn get_key(&self) -> &str {
        self.key.as_str()
    }

    pub(crate) async fn start(self) -> Result<()> {
        let dest_tcp_stream = self.dest_tcp_stream;
        let dest_tcp_framed = Framed::new(dest_tcp_stream, BytesCodec::new());
        let (dest_tcp_framed_write, dest_tcp_framed_read) = dest_tcp_framed.split();
        let agent_message_framed_write = self.agent_message_framed_write;
        let agent_message_framed_read = self.agent_message_framed_read;
        let user_token = self.user_token;
        let key = self.key;
        let agent_connection_id = self.agent_connection_id;
        let dest_to_agent_guard = Self::start_dest_to_agent_task(
            agent_connection_id.clone(),
            key.clone(),
            agent_message_framed_write,
            dest_tcp_framed_read,
            &user_token,
        );
        let agent_to_dest_guard = Self::start_agent_to_dest_task(agent_connection_id.clone(), key.clone(), agent_message_framed_read, dest_tcp_framed_write);
        if let Err(e) = tokio::try_join!(dest_to_agent_guard, agent_to_dest_guard) {
            error!("Agent connection [{agent_connection_id}] for tcp loop [{key}] fail to do relay process becuase of error: {e:?}")
        };
        Ok(())
    }
}
