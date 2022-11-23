use crate::common::{AgentMessageFramed, ProxyServerPayloadEncryptionSelector};
use anyhow::{Context, Result};
use bytes::BytesMut;
use futures::{stream::SplitStream, StreamExt};
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use ppaass_common::generate_uuid;
use ppaass_protocol::{PpaassMessage, PpaassMessagePayloadEncryptionSelector, PpaassMessageUtil, PpaassNetAddress};
use pretty_hex::pretty_hex;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::codec::{BytesCodec, Framed};
use tracing::{debug, error, trace};

type AgentMessageFramedWrite = Arc<Mutex<SplitSink<AgentMessageFramed, PpaassMessage>>>;
type DestTcpFramedWrite = SplitSink<Framed<TcpStream, BytesCodec>, BytesMut>;
type DestTcpFramedRead = SplitStream<Framed<TcpStream, BytesCodec>>;

#[derive(Debug)]
pub(crate) struct TcpSession {
    dest_tcp_framed_write: Option<DestTcpFramedWrite>,
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
        let dest_tcp_framed = Framed::with_capacity(dest_tcp_stream, BytesCodec::new(), 1024 * 64);
        let key = Self::generate_key(&agent_address, &src_address, &dest_address);
        // let agent_message_framed_write_for_read_task = agent_message_framed_write.clone();
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_initialize_success_message = PpaassMessageUtil::create_proxy_tcp_session_initialize_success_response(
            &user_token,
            key.clone(),
            src_address.clone(),
            dest_address.clone(),
            payload_encryption_token,
        )?;
        let agent_message_framed_write_for_read_dest = agent_message_framed_write.clone();
        let mut agent_message_framed_write = agent_message_framed_write.lock().await;
        agent_message_framed_write
            .send(tcp_initialize_success_message)
            .await
            .context("Fail to send tcp initialize success message to agent")?;
        let (dest_tcp_framed_write, dest_tcp_framed_read) = dest_tcp_framed.split::<BytesMut>();
        let dest_read_guard = Self::start_dest_read_task(
            agent_message_framed_write_for_read_dest,
            dest_tcp_framed_read,
            &user_token,
            &key,
            src_address.clone(),
            dest_address.clone(),
        );

        Ok(Self {
            key,
            dest_read_guard,
            dest_tcp_framed_write: Some(dest_tcp_framed_write),
        })
    }

    fn start_dest_read_task(
        agent_message_framed_write: AgentMessageFramedWrite, mut dest_tcp_framed_read: DestTcpFramedRead, user_token: impl AsRef<str>,
        session_key: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
    ) -> JoinHandle<Result<()>> {
        let user_token = user_token.as_ref().to_owned();
        let session_key = session_key.as_ref().to_owned();

        tokio::spawn(async move {
            let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
            while let Some(dest_tcp_data) = dest_tcp_framed_read.next().await {
                let dest_tcp_data = dest_tcp_data?;
                debug!("Session [{session_key}] forward destination data to agent:\n{}\n", pretty_hex(&dest_tcp_data));
                let tcp_relay = PpaassMessageUtil::create_tcp_session_relay_data(
                    &user_token,
                    &session_key,
                    src_address.clone(),
                    dest_address.clone(),
                    payload_encryption_token.clone(),
                    dest_tcp_data.to_vec(),
                    false,
                )?;
                let mut agent_message_framed_write = agent_message_framed_write.lock().await;
                agent_message_framed_write.send(tcp_relay).await?;
            }
            let tcp_relay_complete = PpaassMessageUtil::create_tcp_session_relay_complete(
                &user_token,
                &session_key,
                src_address.clone(),
                dest_address.clone(),
                payload_encryption_token.clone(),
                false,
            )?;
            let mut agent_message_framed_write = agent_message_framed_write.lock().await;
            agent_message_framed_write.send(tcp_relay_complete).await?;
            debug!("Session [{session_key}] read destination data complete");
            Ok(())
        })
    }

    pub(crate) async fn forward(&mut self, data: impl AsRef<[u8]>) -> Result<()> {
        let Some(dest_tcp_framed_write) = self.dest_tcp_framed_write.as_mut() else{
            return Err(anyhow::anyhow!("No dest tcp stream existing in current tcp session."));
        };

        let data = BytesMut::from_iter(data.as_ref().to_vec());
        debug!("Session [{}] forward agent data to destination:\n{}\n", self.key, pretty_hex(&data));

        dest_tcp_framed_write.send(data).await.context("Fail to forward agent data to destination")?;
        Ok(())
    }

    pub(crate) fn get_key(&self) -> &str {
        self.key.as_str()
    }
}

impl Drop for TcpSession {
    fn drop(&mut self) {
        drop(self.dest_tcp_framed_write.take());
        self.dest_read_guard.abort();
    }
}
