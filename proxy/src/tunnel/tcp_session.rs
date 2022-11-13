use crate::common::AgentMessageFramed;
use anyhow::Result;
use futures_util::stream::SplitSink;
use ppaass_protocol::{PpaassMessage, PpaassNetAddress};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

type AgentMessageFramedWrite = Arc<Mutex<SplitSink<AgentMessageFramed, PpaassMessage>>>;

#[derive(Debug)]
pub(crate) struct TcpSession {
    dest_tcp_stream_write: Option<OwnedWriteHalf>,
    dest_read_guard: JoinHandle<Result<()>>,
    user_token: String,
    src_address: PpaassNetAddress,
    dest_address: PpaassNetAddress,
}

impl TcpSession {
    pub(crate) async fn new(
        agent_message_framed_write: AgentMessageFramedWrite, user_token: impl AsRef<str>, src_address: PpaassNetAddress, dest_address: PpaassNetAddress,
    ) -> Result<Self> {
        let socket_address = dest_address.to_socket_addrs()?;
        let socket_address = socket_address.collect::<Vec<SocketAddr>>();
        let mut dest_tcp_stream = match TcpStream::connect(socket_address.as_slice()).await {
            Ok(stream) => stream,
            Err(e) => {
                let mut agent_message_framed_write = agent_message_framed_write.lock().await;
                return Err(anyhow::anyhow!(e));
            },
        };
        let (dest_tcp_stream_read, dest_tcp_stream_write) = dest_tcp_stream.into_split();
        let dest_read_guard = Self::start_dest_read_task(agent_message_framed_write, dest_tcp_stream_read);
        Ok(Self {
            dest_read_guard,
            dest_tcp_stream_write: Some(dest_tcp_stream_write),
            user_token: user_token.as_ref().to_string(),
            src_address,
            dest_address,
        })
    }

    fn start_dest_read_task(agent_message_framed_write: AgentMessageFramedWrite, mut dest_tcp_stream_read: OwnedReadHalf) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            loop {
                let mut dest_read_buf = Vec::<u8>::with_capacity(1024 * 64);
                let dest_tcp_stream_read_size = dest_tcp_stream_read.read(&mut dest_read_buf).await?;
            }
            Ok(())
        })
    }

    pub(crate) async fn forward(&mut self, data: &Vec<u8>) -> Result<()> {
        let Some(dest_tcp_stream_write) = self.dest_tcp_stream_write.as_mut() else{
            return Err(anyhow::anyhow!("No dest tcp stream existing in current tcp session."));
        };
        dest_tcp_stream_write.write(data).await?;
        Ok(())
    }
}

impl Drop for TcpSession {
    fn drop(&mut self) {
        drop(self.dest_tcp_stream_write.take());
        self.dest_read_guard.abort();
    }
}
