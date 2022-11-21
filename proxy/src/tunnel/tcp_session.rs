use crate::common::{AgentMessageFramed, ProxyServerPayloadEncryptionSelector};
use anyhow::{Context, Result};
use bytes::Buf;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use ppaass_common::generate_uuid;
use ppaass_protocol::{PpaassMessage, PpaassMessagePayloadEncryptionSelector, PpaassMessageUtil, PpaassNetAddress};
use pretty_hex::pretty_hex;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{error, trace};
type AgentMessageFramedWrite = Arc<Mutex<SplitSink<AgentMessageFramed, PpaassMessage>>>;

#[derive(Debug)]
pub(crate) struct TcpSession {
    dest_tcp_stream_write: Option<OwnedWriteHalf>,
    dest_read_guard: JoinHandle<Result<()>>,
    key: String,
}

impl TcpSession {
    fn generate_key(agent_address: &PpaassNetAddress, src_address: &PpaassNetAddress, dest_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]::[{src_address}=>{dest_address}]")
    }
    pub(crate) async fn new(
        agent_message_framed_write: AgentMessageFramedWrite, user_token: impl AsRef<str>, agent_address: PpaassNetAddress, src_address: PpaassNetAddress,
        dest_address: PpaassNetAddress,
    ) -> Result<Self> {
        let user_token = user_token.as_ref().to_owned();
        let socket_address = dest_address.to_socket_addrs().context("Convert destination address to socket address")?;
        let socket_address = socket_address.collect::<Vec<SocketAddr>>();
        let dest_tcp_stream = match TcpStream::connect(socket_address.as_slice()).await.context("Connect to destination fail") {
            Ok(stream) => stream,
            Err(e) => {
                error!("Fail connect to dest address because of error: {e:?}");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_initialize_fail_message =
                    PpaassMessageUtil::create_proxy_tcp_session_initialize_fail_response(&user_token, src_address, dest_address, payload_encryption_token)?;
                let mut agent_message_framed_write = agent_message_framed_write.lock().await;
                agent_message_framed_write
                    .send(tcp_initialize_fail_message)
                    .await
                    .context("Fail to send tcp initialize fail message to agent")?;
                return Err(anyhow::anyhow!(e));
            },
        };
        let key = Self::generate_key(&agent_address, &src_address, &dest_address);
        let agent_message_framed_write_for_read_task = agent_message_framed_write.clone();
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_initialize_success_message = PpaassMessageUtil::create_proxy_tcp_session_initialize_success_response(
            &user_token,
            key.clone(),
            src_address.clone(),
            dest_address.clone(),
            payload_encryption_token,
        )?;
        let mut agent_message_framed_write = agent_message_framed_write.lock().await;
        agent_message_framed_write
            .send(tcp_initialize_success_message)
            .await
            .context("Fail to send tcp initialize success message to agent")?;
        let (dest_tcp_stream_read, dest_tcp_stream_write) = dest_tcp_stream.into_split();
        let dest_read_guard = Self::start_dest_read_task(
            agent_message_framed_write_for_read_task,
            dest_tcp_stream_read,
            &user_token,
            &key,
            src_address.clone(),
            dest_address.clone(),
        );

        Ok(Self {
            key,
            dest_read_guard,
            dest_tcp_stream_write: Some(dest_tcp_stream_write),
        })
    }

    fn start_dest_read_task(
        agent_message_framed_write: AgentMessageFramedWrite, mut dest_tcp_stream_read: OwnedReadHalf, user_token: impl AsRef<str>,
        session_key: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
    ) -> JoinHandle<Result<()>> {
        let user_token = user_token.as_ref().to_owned();
        let session_key = session_key.as_ref().to_owned();

        tokio::spawn(async move {
            let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
            loop {
                let mut dest_read_buf = Vec::<u8>::with_capacity(1024 * 64);
                let dest_tcp_stream_read_size = dest_tcp_stream_read.read(&mut dest_read_buf).await?;
                let concrete_dest_inbound_data = &dest_read_buf[..dest_tcp_stream_read_size];
                let tcp_relay = PpaassMessageUtil::create_tcp_session_relay(
                    &user_token,
                    &session_key,
                    src_address.clone(),
                    dest_address.clone(),
                    payload_encryption_token.clone(),
                    concrete_dest_inbound_data.to_vec(),
                    false,
                )?;
                let mut agent_message_framed_write = agent_message_framed_write.lock().await;
                agent_message_framed_write.send(tcp_relay).await?;
                trace!(
                    "Session [{session_key}] forward agent data to destination:\n{}\n",
                    pretty_hex(&concrete_dest_inbound_data)
                );
            }
        })
    }

    pub(crate) async fn forward(&mut self, mut data: impl Buf + AsRef<[u8]>) -> Result<()> {
        let Some(dest_tcp_stream_write) = self.dest_tcp_stream_write.as_mut() else{
            return Err(anyhow::anyhow!("No dest tcp stream existing in current tcp session."));
        };
        dest_tcp_stream_write
            .write_all_buf(&mut data)
            .await
            .context("Fail to forward agent data to destination")?;
        trace!("Session [{}] forward agent data to destination:\n{}\n", self.key, pretty_hex(&data));
        Ok(())
    }

    pub(crate) fn get_key(&self) -> &str {
        &self.key.as_str()
    }
}

impl Drop for TcpSession {
    fn drop(&mut self) {
        drop(self.dest_tcp_stream_write.take());
        self.dest_read_guard.abort();
    }
}
