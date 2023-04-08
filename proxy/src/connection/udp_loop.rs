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
    udp_socket: Arc<UdpSocket>,
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
        let loop_key = self.loop_key;
        let agent_connection_id = self.agent_connection_id;
        let agent_connection_write = Arc::new(Mutex::new(self.agent_connection_write));
        let user_token = self.user_token;

        loop {
            let agent_message = match self.agent_connection_read.next().await {
                Some(Ok(agent_message)) => agent_message,
                Some(Err(e)) => {
                    error!("Agent connection [{agent_connection_id}] with udp loop {loop_key} fail to read agent message because of error: {e:?}");
                    return Err(anyhow!(e));
                },
                None => return Ok(()),
            };
            let PpaassMessageParts { payload_bytes, .. } = agent_message.split();
            let UdpLoopDataParts {
                src_address,
                dst_address,
                raw_data_bytes,
            } = TryInto::<UdpLoopData>::try_into(payload_bytes)?.split();
            let agent_connection_write = agent_connection_write.clone();
            let user_token = user_token.clone();
            let agent_connection_id = agent_connection_id.clone();
            let loop_key = loop_key.clone();
            let dst_udp_socket = self.udp_socket.clone();
            tokio::spawn(async move {
                let dst_udp_socket_address = match dst_address.to_socket_addrs() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Agent connection [{agent_connection_id}] with udp loop {loop_key} fail to convert destination address [{dst_address}] because of error: {e:?}");
                        return;
                    },
                };
                info!(
                    "Agent connection [{agent_connection_id}] receive agent udp data:\n{}\n",
                    pretty_hex(&raw_data_bytes)
                );
                let dst_socket_address = dst_udp_socket_address.collect::<Vec<SocketAddr>>();
                if let Err(e) = dst_udp_socket.connect(dst_socket_address.as_slice()).await {
                    error!(
                        "Agent connection [{agent_connection_id}] with udp loop {loop_key} fail to connect udp socket to [{dst_address}] because of error: {e:?}"
                    );
                    return;
                };
                if let Err(e) = dst_udp_socket.send(&raw_data_bytes).await {
                    error!("Agent connection [{agent_connection_id}] with udp loop {loop_key} fail to send data to udp socket because of error: {e:?}");
                    return;
                };
                info!("Agent connection [{agent_connection_id}] waiting for receive destination udp data, destination address: {dst_address}");
                let mut dst_udp_recv_buf = [0u8; 65535];
                let dst_udp_data_size = match dst_udp_socket.recv(&mut dst_udp_recv_buf).await {
                    Ok(size) => size,
                    Err(e) => {
                        error!(
                            "Agent connection [{agent_connection_id}] with udp loop {loop_key} fail to receive data from udp socket because of error: {e:?}"
                        );
                        return;
                    },
                };
                let dst_udp_recv_buf = &dst_udp_recv_buf[..dst_udp_data_size];
                info!(
                    "Agent connection [{agent_connection_id}] receive destination udp data:\n{}\n",
                    pretty_hex(&dst_udp_recv_buf)
                );
                let mut agent_connection_write = agent_connection_write.lock().await;
                let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let udp_loop_data_message =
                    match PpaassMessageGenerator::generate_udp_loop_data(user_token, payload_encryption, src_address, dst_address, dst_udp_recv_buf.to_vec()) {
                        Ok(udp_loop_data_message) => udp_loop_data_message,
                        Err(e) => {
                            error!(
                            "Agent connection [{agent_connection_id}] with udp loop {loop_key} fail to generate udp loop data message because of error: {e:?}"
                        );
                            return;
                        },
                    };
                if let Err(e) = agent_connection_write.send(udp_loop_data_message).await {
                    error!("Agent connection [{agent_connection_id}] with udp loop {loop_key} fail to send udp loop data to agent because of error: {e:?}");
                }
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
    udp_socket: Arc<UdpSocket>,
}

impl<T, R, I> UdpLoopBuilder<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn new(udp_socket: Arc<UdpSocket>) -> Self {
        Self {
            udp_socket,
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
        let udp_initialize_success_message = PpaassMessageGenerator::generate_udp_loop_init_success_response(&loop_key, &user_token, payload_encryption_token)?;
        if let Err(e) = agent_connection_write.send(udp_initialize_success_message).await {
            error!("Agent connection [{agent_connection_id}] fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow!(e));
        };

        Ok(UdpLoop {
            udp_socket: self.udp_socket,
            loop_key,
            agent_connection_read,
            agent_connection_write,
            user_token,
            agent_connection_id,
        })
    }
}
