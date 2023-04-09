use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use futures::{SinkExt, StreamExt};
use ppaass_common::{
    generate_uuid,
    udp_loop::{UdpLoopData, UdpLoopDataParts},
    PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress,
    RsaCryptoFetcher,
};
use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    sync::Mutex,
};
use tracing::{error, info};

use crate::common::ProxyServerPayloadEncryptionSelector;

use anyhow::{anyhow, Context, Result};

#[derive(Debug)]
pub(crate) struct UdpLoop<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    agent_connection_read: PpaassConnectionRead<T, R, I>,
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    loop_key: String,
    user_token: String,
    agent_connection_id: String,
}

impl<T, R, I> UdpLoop<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn generate_loop_key(agent_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]")
    }

    pub(crate) fn get_loop_key(&self) -> &str {
        self.loop_key.as_str()
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let agent_connection_write = Arc::new(Mutex::new(self.agent_connection_write));
        loop {
            let loop_key = self.loop_key.clone();
            let agent_connection_id = self.agent_connection_id.clone();

            let agent_message = match self.agent_connection_read.next().await {
                Some(Ok(agent_message)) => agent_message,
                Some(Err(e)) => {
                    error!("Agent connection {agent_connection_id} with udp loop {loop_key} fail to read agent message because of error: {e:?}");
                    return Err(anyhow!(e));
                },
                None => return Ok(()),
            };
            let user_token = self.user_token.clone();
            let agent_connection_write = agent_connection_write.clone();
            tokio::spawn(async move {
                let dst_udp_socket = UdpSocket::bind("0.0.0.0:0").await?;
                let PpaassMessageParts { payload_bytes, .. } = agent_message.split();
                let UdpLoopDataParts {
                    src_address,
                    dst_address,
                    raw_data_bytes,
                } = TryInto::<UdpLoopData>::try_into(payload_bytes)?.split();
                let agent_connection_id = agent_connection_id.clone();

                let dst_socket_addrs = match dst_address.to_socket_addrs() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Agent connection {agent_connection_id} with udp loop {loop_key} fail to convert destination address [{dst_address}] because of error: {e:?}");
                        return Err(anyhow!(e));
                    },
                };
                info!(
                    "Agent connection {agent_connection_id} receive agent udp data:\n{}\n",
                    pretty_hex(&raw_data_bytes)
                );
                let dst_socket_addrs = dst_socket_addrs.collect::<Vec<SocketAddr>>();
                if let Err(e) = dst_udp_socket.send_to(&raw_data_bytes, dst_socket_addrs.as_slice()).await {
                    error!("Agent connection {agent_connection_id} with udp loop {loop_key} fail to send data to udp socket because of error: {e:?}");
                };
                let mut dst_recv_buf = [0u8; 65535];
                let (data_size, dst_socket_addr) = match dst_udp_socket.recv_from(&mut dst_recv_buf).await {
                    Ok(size) => size,
                    Err(e) => {
                        error!("Agent connection {agent_connection_id} with udp loop {loop_key} fail to receive data from udp socket because of error: {e:?}");
                        return Err(anyhow!(e));
                    },
                };
                let dst_address: PpaassNetAddress = dst_socket_addr.into();
                let dst_recv_buf = &dst_recv_buf[..data_size];
                info!(
                    "Agent connection {agent_connection_id} receive destination udp data:\n{}\n",
                    pretty_hex(&dst_recv_buf)
                );

                let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));

                let data_message = match PpaassMessageGenerator::generate_udp_loop_data(
                    user_token.clone(),
                    payload_encryption,
                    src_address,
                    dst_address,
                    dst_recv_buf.to_vec(),
                ) {
                    Ok(data_message) => data_message,
                    Err(e) => {
                        error!(
                            "Agent connection {agent_connection_id} with udp loop {loop_key} fail to generate udp loop data message because of error: {e:?}"
                        );
                        return Err(anyhow!(e));
                    },
                };
                let mut agent_connection_write = agent_connection_write.lock().await;
                if let Err(e) = agent_connection_write.send(data_message).await {
                    error!("Agent connection {agent_connection_id} with udp loop {loop_key} fail to send udp loop data to agent because of error: {e:?}");
                    return Err(anyhow!(e));
                };
                Ok(())
            });
        }
    }
}

pub(crate) struct UdpLoopBuilder<T, R, I>
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
}

impl<T, R, I> UdpLoopBuilder<T, R, I>
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
        }
    }
    pub(crate) fn agent_connection_id(mut self, agent_connection_id: impl AsRef<str>) -> UdpLoopBuilder<T, R, I> {
        self.agent_connection_id = Some(agent_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> UdpLoopBuilder<T, R, I> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> UdpLoopBuilder<T, R, I> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn agent_connection_read(mut self, agent_connection_read: PpaassConnectionRead<T, R, I>) -> UdpLoopBuilder<T, R, I> {
        self.agent_connection_read = Some(agent_connection_read);
        self
    }

    pub(crate) fn agent_connection_write(mut self, agent_connection_write: PpaassConnectionWrite<T, R, I>) -> UdpLoopBuilder<T, R, I> {
        self.agent_connection_write = Some(agent_connection_write);
        self
    }

    pub(crate) async fn build(self) -> Result<UdpLoop<T, R, I>> {
        let agent_connection_id = self.agent_connection_id.context("Agent connection id not assigned for tcp loop builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for tcp loop builder")?;
        let user_token = self.user_token.context("User token not assigned for tcp loop builder")?;
        let loop_key = UdpLoop::<T, R, I>::generate_loop_key(&agent_address);
        let mut agent_connection_write = self
            .agent_connection_write
            .context("Agent message framed write not assigned for tcp loop builder")?;
        let agent_connection_read = self
            .agent_connection_read
            .context("Agent message framed read not assigned for tcp loop builder")?;
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let init_success_message = PpaassMessageGenerator::generate_udp_loop_init_success_response(&loop_key, &user_token, payload_encryption_token)?;
        if let Err(e) = agent_connection_write.send(init_success_message).await {
            error!("Agent connection {agent_connection_id} fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow!(e));
        };
        Ok(UdpLoop {
            loop_key,
            agent_connection_read,
            agent_connection_write,
            user_token,
            agent_connection_id,
        })
    }
}
