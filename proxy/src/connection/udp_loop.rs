use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use futures::{SinkExt, StreamExt};
use ppaass_common::{
    generate_uuid,
    udp_loop::{UdpLoopData, UdpLoopDataParts},
    PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress, RsaCryptoFetcher,
};
use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    sync::Mutex,
};
use tracing::{error, info};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig};

use super::{AgentConnectionRead, AgentConnectionWrite};
use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) struct UdpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    agent_connection_read: AgentConnectionRead<T, R>,
    agent_connection_write: AgentConnectionWrite<T, R>,
    key: String,
    user_token: String,
    agent_connection_id: String,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R> UdpLoop<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    fn generate_key(agent_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]")
    }

    pub(crate) fn get_key(&self) -> &str {
        self.key.as_str()
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let key = self.key;
        let agent_connection_id = self.agent_connection_id;
        let agent_connection_write = Arc::new(Mutex::new(self.agent_connection_write));
        let user_token = self.user_token;

        loop {
            let agent_message = match self.agent_connection_read.next().await {
                Some(Ok(agent_message)) => agent_message,
                Some(Err(e)) => {
                    error!("Agent connection [{agent_connection_id}] with udp loop [{key}] fail to read agent message because of error: {e:?}");
                    return Err(anyhow::anyhow!(e));
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
            let key = key.clone();
            tokio::spawn(async move {
                let udp_socket = match UdpSocket::bind("0.0.0.0:0").await {
                    Ok(udp_socket) => udp_socket,
                    Err(e) => {
                        error!("Agent connection [{agent_connection_id}] with udp loop [{key}] fail to bind udp socket because of error: {e:?}");
                        return;
                    },
                };
                let dest_socket_address = match dst_address.to_socket_addrs() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Agent connection [{agent_connection_id}] with udp loop [{key}] fail to convert destination address [{dst_address}] because of error: {e:?}");
                        return;
                    },
                };
                info!(
                    "Agent connection [{agent_connection_id}] receive agent udp data:\n{}\n",
                    pretty_hex(&raw_data_bytes)
                );
                let dest_socket_address = dest_socket_address.collect::<Vec<SocketAddr>>();
                if let Err(e) = udp_socket.connect(dest_socket_address.as_slice()).await {
                    error!("Agent connection [{agent_connection_id}] with udp loop [{key}] fail to connect udp socket to [{dest_socket_address:?}] because of error: {e:?}");
                    return;
                };
                if let Err(e) = udp_socket.send(&raw_data_bytes).await {
                    error!("Agent connection [{agent_connection_id}] with udp loop [{key}] fail to send data to udp socket because of error: {e:?}");
                    return;
                };
                info!("Agent connection [{agent_connection_id}] waiting for receive destination udp data");
                let mut dst_udp_recv_buf = [0u8; 65535];
                let dst_udp_data_size = match udp_socket.recv(&mut dst_udp_recv_buf).await {
                    Ok(size) => size,
                    Err(e) => {
                        error!("Agent connection [{agent_connection_id}] with udp loop [{key}] fail to receive data from udp socket because of error: {e:?}");
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
                                "Agent connection [{agent_connection_id}] with udp loop [{key}] fail to generate udp loop data message because of error: {e:?}"
                            );
                            return;
                        },
                    };
                if let Err(e) = agent_connection_write.send(udp_loop_data_message).await {
                    error!("Agent connection [{agent_connection_id}] with udp loop [{key}] fail to send udp loop data to agent because of error: {e:?}");
                }
            });
        }
    }
}

pub(crate) struct UdpLoopBuilder<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    agent_connection_id: Option<String>,
    agent_connection_read: Option<AgentConnectionRead<T, R>>,
    agent_connection_write: Option<AgentConnectionWrite<T, R>>,
    user_token: Option<String>,
    agent_address: Option<PpaassNetAddress>,
}

impl<T, R> UdpLoopBuilder<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
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
    pub(crate) fn agent_connection_id(mut self, agent_connection_id: impl AsRef<str>) -> UdpLoopBuilder<T, R> {
        self.agent_connection_id = Some(agent_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> UdpLoopBuilder<T, R> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> UdpLoopBuilder<T, R> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn agent_connection_read(mut self, agent_connection_read: AgentConnectionRead<T, R>) -> UdpLoopBuilder<T, R> {
        self.agent_connection_read = Some(agent_connection_read);
        self
    }

    pub(crate) fn agent_connection_write(mut self, agent_connection_write: AgentConnectionWrite<T, R>) -> UdpLoopBuilder<T, R> {
        self.agent_connection_write = Some(agent_connection_write);
        self
    }

    pub(crate) async fn build(self, configuration: Arc<ProxyServerConfig>) -> Result<UdpLoop<T, R>> {
        let agent_connection_id = self.agent_connection_id.context("Agent connection id not assigned for tcp loop builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for tcp loop builder")?;
        let user_token = self.user_token.context("User token not assigned for tcp loop builder")?;
        let key = UdpLoop::<T, R>::generate_key(&agent_address);
        let mut agent_connection_write = self
            .agent_connection_write
            .context("Agent message framed write not assigned for tcp loop builder")?;
        let agent_connection_read = self
            .agent_connection_read
            .context("Agent message framed read not assigned for tcp loop builder")?;

        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let udp_initialize_success_message = PpaassMessageGenerator::generate_udp_loop_init_success_response(&key, &user_token, payload_encryption_token)?;
        if let Err(e) = agent_connection_write.send(udp_initialize_success_message).await {
            error!("Agent connection [{agent_connection_id}] fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow::anyhow!(e));
        };
        Ok(UdpLoop {
            key,
            agent_connection_read,
            agent_connection_write,
            user_token,
            agent_connection_id,
            configuration,
        })
    }
}
