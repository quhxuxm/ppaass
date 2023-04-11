use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use futures::{SinkExt, StreamExt};
use ppaass_common::{
    generate_uuid,
    udp::{UdpData, UdpDataParts, UdpInitResponseType},
    PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageGenerator, PpaassMessageParts, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress,
    RsaCryptoFetcher,
};
use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    sync::Mutex,
    time::timeout,
};
use tracing::{debug, error, info};

use crate::common::ProxyServerPayloadEncryptionSelector;

use anyhow::{anyhow, Context, Result};

pub(crate) struct UdpHandlerBuilder<T, R, I>
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

impl<T, R, I> UdpHandlerBuilder<T, R, I>
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
    pub(crate) fn agent_connection_id(mut self, agent_connection_id: impl AsRef<str>) -> UdpHandlerBuilder<T, R, I> {
        self.agent_connection_id = Some(agent_connection_id.as_ref().to_owned());
        self
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> UdpHandlerBuilder<T, R, I> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> UdpHandlerBuilder<T, R, I> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn agent_connection_read(mut self, agent_connection_read: PpaassConnectionRead<T, R, I>) -> UdpHandlerBuilder<T, R, I> {
        self.agent_connection_read = Some(agent_connection_read);
        self
    }

    pub(crate) fn agent_connection_write(mut self, agent_connection_write: PpaassConnectionWrite<T, R, I>) -> UdpHandlerBuilder<T, R, I> {
        self.agent_connection_write = Some(agent_connection_write);
        self
    }

    pub(crate) async fn build(self) -> Result<UdpHandler<T, R, I>> {
        let agent_connection_id = self.agent_connection_id.context("Agent connection id not assigned for udp handler builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for udp handler builder")?;
        let user_token = self.user_token.context("User token not assigned for udp handler builder")?;
        let handler_key = UdpHandler::<T, R, I>::generate_key(&agent_address);
        let mut agent_connection_write = self
            .agent_connection_write
            .context("Agent message framed write not assigned for udp handler builder")?;
        let agent_connection_read = self
            .agent_connection_read
            .context("Agent message framed read not assigned for udp handler builder")?;
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let udp_init_success_message =
            PpaassMessageGenerator::generate_udp_init_response(&handler_key, &user_token, payload_encryption_token, UdpInitResponseType::Success)?;
        if let Err(e) = agent_connection_write.send(udp_init_success_message).await {
            error!("Agent connection {agent_connection_id} fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow!(e));
        };
        Ok(UdpHandler {
            handler_key,
            agent_connection_read,
            agent_connection_write,
            user_token,
            agent_connection_id,
        })
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct UdpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    agent_connection_read: PpaassConnectionRead<T, R, I>,
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: String,
    user_token: String,
    agent_connection_id: String,
}

impl<T, R, I> UdpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn generate_key(agent_address: &PpaassNetAddress) -> String {
        format!("[{agent_address}]")
    }

    pub(crate) fn get_key(&self) -> &str {
        self.handler_key.as_str()
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let agent_connection_write = Arc::new(Mutex::new(self.agent_connection_write));
        let dst_udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        loop {
            let handler_key = self.handler_key.clone();
            let agent_connection_id = self.agent_connection_id.clone();

            let agent_message = match self.agent_connection_read.next().await {
                Some(Ok(agent_message)) => agent_message,
                Some(Err(e)) => {
                    error!("Agent connection {agent_connection_id} with udp loop {handler_key} fail to read agent message because of error: {e:?}");
                    return Err(anyhow!(e));
                },
                None => {
                    debug!("Agent connection {agent_connection_id} with udp loop {handler_key} complete to read agent message from TCP.");
                    return Ok(());
                },
            };
            let user_token = self.user_token.clone();
            let agent_connection_write = agent_connection_write.clone();
            let dst_udp_socket = dst_udp_socket.clone();
            tokio::spawn(async move {
                let mut agent_connection_write = agent_connection_write.lock().await;
                let PpaassMessageParts { payload, .. } = agent_message.split();
                let UdpDataParts {
                    src_address,
                    dst_address,
                    raw_data,
                } = TryInto::<UdpData>::try_into(payload)?.split();
                let agent_connection_id = agent_connection_id.clone();

                let dst_socket_addrs = match dst_address.to_socket_addrs() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Agent connection {agent_connection_id} with udp loop {handler_key} fail to convert destination address [{dst_address}] because of error: {e:?}");
                        return Err(anyhow!(e));
                    },
                };
                info!("Agent connection {agent_connection_id} receive agent udp data:\n{}\n", pretty_hex(&raw_data));
                let dst_socket_addrs = dst_socket_addrs.collect::<Vec<SocketAddr>>();
                if let Err(e) = dst_udp_socket.send_to(&raw_data, dst_socket_addrs.as_slice()).await {
                    error!("Agent connection {agent_connection_id} with udp loop {handler_key} fail to send data to udp socket because of error: {e:?}");
                };
                let mut dst_recv_buf = [0u8; 65535];
                let (data_size, dst_socket_addr) = match timeout(Duration::from_secs(2), dst_udp_socket.recv_from(&mut dst_recv_buf)).await {
                    Ok(Ok(data_size)) => data_size,
                    Ok(Err(e)) => {
                        error!(
                            "Agent connection {agent_connection_id} with udp loop {handler_key} fail to receive data from udp socket because of error: {e:?}"
                        );
                        return Err(anyhow!(e));
                    },
                    Err(e) => {
                        error!("Agent connection {agent_connection_id} with udp loop {handler_key} fail to receive data from udp socket because of timeout");
                        return Err(anyhow!(e));
                    },
                };
                let dst_address: PpaassNetAddress = dst_socket_addr.into();

                let dst_recv_buf = &dst_recv_buf[..data_size];
                info!(
                    "Agent connection {agent_connection_id} with udp loop {handler_key} receive destination udp data:\n{}\n",
                    pretty_hex(&dst_recv_buf)
                );

                let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));

                let udp_data_message =
                    match PpaassMessageGenerator::generate_udp_data(user_token.clone(), payload_encryption, src_address, dst_address, dst_recv_buf.to_vec()) {
                        Ok(data_message) => data_message,
                        Err(e) => {
                            error!(
                            "Agent connection {agent_connection_id} with udp loop {handler_key} fail to generate udp loop data message because of error: {e:?}"
                        );
                            return Err(anyhow!(e));
                        },
                    };

                if let Err(e) = agent_connection_write.send(udp_data_message).await {
                    error!("Agent connection {agent_connection_id} with udp loop {handler_key} fail to send udp loop data to agent because of error: {e:?}");
                    return Err(anyhow!(e));
                };
                Ok(())
            });
        }
    }
}
