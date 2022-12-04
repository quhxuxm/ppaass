use crate::common::ProxyServerPayloadEncryptionSelector;
use anyhow::{Context, Result};

use futures::StreamExt;

use futures_util::SinkExt;
use ppaass_common::{generate_uuid, PpaassMessageParts, RsaCryptoFetcher};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use std::net::{SocketAddr, ToSocketAddrs};

use tokio::io::AsyncWriteExt;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio::{
    io::{AsyncReadExt, ReadHalf, WriteHalf},
    task::JoinHandle,
};

use tracing::error;

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
        agent_message_framed_read: AgentMessageFramedRead<T, R>, mut agent_message_framed_write: AgentMessageFramedWrite<T, R>, user_token: impl AsRef<str>,
        agent_address: PpaassNetAddress, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
    ) -> Result<Self> {
        let key = Self::generate_key(&agent_address, &src_address, &dest_address);
        let user_token = user_token.as_ref().to_owned();
        let socket_address = dest_address.to_socket_addrs().context("Convert destination address to socket address")?;
        let socket_address = socket_address.collect::<Vec<SocketAddr>>();
        let dest_tcp_stream = match TcpStream::connect(socket_address.as_slice()).await.context("Connect to destination fail") {
            Ok(stream) => stream,
            Err(e) => {
                error!("Fail connect to dest address because of error: {e:?}");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_initialize_fail_message =
                    PpaassMessageGenerator::generate_tcp_loop_init_fail_response(&key, &user_token, src_address, dest_address, payload_encryption_token)?;
                agent_message_framed_write
                    .send(tcp_initialize_fail_message)
                    .await
                    .context("Fail to send tcp initialize fail message to agent")?;
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
        agent_message_framed_write
            .send(tcp_initialize_success_message)
            .await
            .context("Fail to send tcp initialize success message to agent")?;

        Ok(Self {
            key,
            agent_message_framed_read,
            agent_message_framed_write,
            dest_tcp_stream,
            user_token,
        })
    }

    fn start_dest_to_agent_task(
        mut agent_message_framed_write: AgentMessageFramedWrite<T, R>, mut dest_tcp_stream_read: ReadHalf<TcpStream>, user_token: impl AsRef<str>,
    ) -> JoinHandle<Result<()>> {
        let user_token = user_token.as_ref().to_owned();
        tokio::spawn(async move {
            let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
            loop {
                let mut buf = Vec::with_capacity(1024 * 64);
                let size = dest_tcp_stream_read.read(&mut buf).await?;
                if size == 0 {
                    return Ok(());
                }
                let buf = &buf[..size];
                let tcp_relay = PpaassMessageGenerator::generate_raw_data(&user_token, payload_encryption_token.clone(), buf.to_vec())?;
                agent_message_framed_write.send(tcp_relay).await?;
            }
        })
    }

    fn start_agent_to_dest_task(
        mut agent_message_framed_read: AgentMessageFramedRead<T, R>, mut dest_tcp_stream_write: WriteHalf<TcpStream>,
    ) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            loop {
                let agent_message = agent_message_framed_read.next().await;
                let Some(agent_message) = agent_message else{
                    return Ok(());
                };
                let PpaassMessageParts { payload_bytes, .. } = agent_message?.split();
                dest_tcp_stream_write.write_all(&payload_bytes).await?;
            }
        })
    }

    pub(crate) fn get_key(&self) -> &str {
        self.key.as_str()
    }

    pub(crate) async fn start(self) -> Result<()> {
        let dest_tcp_stream = self.dest_tcp_stream;
        let (dest_tcp_stream_read, dest_tcp_stream_write) = tokio::io::split(dest_tcp_stream);
        let agent_message_framed_write = self.agent_message_framed_write;
        let agent_message_framed_read = self.agent_message_framed_read;
        let user_token = self.user_token;
        let dest_to_agent_guard = Self::start_dest_to_agent_task(agent_message_framed_write, dest_tcp_stream_read, &user_token);
        let agent_to_dest_guard = Self::start_agent_to_dest_task(agent_message_framed_read, dest_tcp_stream_write);
        let _ = tokio::try_join!(dest_to_agent_guard, agent_to_dest_guard)?;
        Ok(())
    }
}
