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
    ppaass_connection_id: Option<String>,
    ppaass_connection_read: Option<PpaassConnectionRead<T, R, I>>,
    ppaass_connection_write: Option<PpaassConnectionWrite<T, R, I>>,
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
            ppaass_connection_id: None,
            ppaass_connection_read: None,
            ppaass_connection_write: None,
            user_token: None,
            agent_address: None,
        }
    }
    pub(crate) fn ppaass_connection_id(mut self, ppaass_connection_id: impl AsRef<str>) -> UdpHandlerBuilder<T, R, I> {
        self.ppaass_connection_id = Some(ppaass_connection_id.as_ref().to_owned());
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

    pub(crate) fn ppaass_connection_read(mut self, ppaass_connection_read: PpaassConnectionRead<T, R, I>) -> UdpHandlerBuilder<T, R, I> {
        self.ppaass_connection_read = Some(ppaass_connection_read);
        self
    }

    pub(crate) fn ppaass_connection_write(mut self, ppaass_connection_write: PpaassConnectionWrite<T, R, I>) -> UdpHandlerBuilder<T, R, I> {
        self.ppaass_connection_write = Some(ppaass_connection_write);
        self
    }

    fn generate_handler_key(user_token: &str, agent_address: &PpaassNetAddress, ppaass_connection_id: &str) -> String {
        format!("[{ppaass_connection_id}]#[{user_token}]@UDP::[{agent_address}]")
    }
    pub(crate) async fn build(self) -> Result<UdpHandler<T, R, I>> {
        let ppaass_connection_id = self.ppaass_connection_id.context("Agent connection id not assigned for udp handler builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for udp handler builder")?;
        let user_token = self.user_token.context("User token not assigned for udp handler builder")?;
        let handler_key = Self::generate_handler_key(&user_token, &agent_address, &ppaass_connection_id);
        let ppaass_connection_write = self
            .ppaass_connection_write
            .context("Agent message framed write not assigned for udp handler builder")?;
        let ppaass_connection_read = self
            .ppaass_connection_read
            .context("Agent message framed read not assigned for udp handler builder")?;
        Ok(UdpHandler {
            handler_key,
            ppaass_connection_read,
            ppaass_connection_write,
            user_token,
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
    ppaass_connection_read: PpaassConnectionRead<T, R, I>,
    ppaass_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: String,
    user_token: String,
}

impl<T, R, I> UdpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) async fn exec(self) -> Result<()> {
        let mut ppaass_connection_write = self.ppaass_connection_write;
        let mut ppaass_connection_read = self.ppaass_connection_read;
        let user_token = self.user_token;
        let handler_key = self.handler_key;
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let udp_init_success_message =
            PpaassMessageGenerator::generate_udp_init_response(&handler_key, &user_token, payload_encryption_token, UdpInitResponseType::Success)?;
        if let Err(e) = ppaass_connection_write.send(udp_init_success_message).await {
            error!("Udp handler {handler_key} fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow!(e));
        };

        let ppaass_connection_write = Arc::new(Mutex::new(ppaass_connection_write));
        let dst_udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        loop {
            let agent_message = match ppaass_connection_read.next().await {
                Some(Ok(agent_message)) => agent_message,
                Some(Err(e)) => {
                    error!("Udp handler {handler_key} fail to read agent message because of error: {e:?}");
                    return Err(anyhow!(e));
                },
                None => {
                    debug!("Udp handler {handler_key} complete to read agent message from TCP.");
                    return Ok(());
                },
            };
            let ppaass_connection_write = ppaass_connection_write.clone();
            let handler_key = handler_key.clone();
            let user_token = user_token.clone();
            let dst_udp_socket = dst_udp_socket.clone();
            tokio::spawn(async move {
                let mut ppaass_connection_write = ppaass_connection_write.lock().await;
                let PpaassMessageParts { payload, .. } = agent_message.split();
                let UdpDataParts {
                    src_address,
                    dst_address,
                    raw_data,
                } = TryInto::<UdpData>::try_into(payload)?.split();

                let dst_socket_addrs = match dst_address.to_socket_addrs() {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Udp handler {handler_key} fail to convert destination address [{dst_address}] because of error: {e:?}");
                        return Err(anyhow!(e));
                    },
                };
                info!(
                    "Udp handler {handler_key} receive agent udp data from [{dst_address}]:\n{}\n",
                    pretty_hex(&raw_data)
                );
                let dst_socket_addrs = dst_socket_addrs.collect::<Vec<SocketAddr>>();
                // dst_udp_socket.connect(dst_socket_addrs.as_slice()).await?;
                if let Err(e) = dst_udp_socket.send_to(&raw_data, dst_socket_addrs.as_slice()).await {
                    error!("Udp handler {handler_key} fail to send data to udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
                    return Err(anyhow!(e));
                };
                let mut dst_recv_buf = [0u8; 65535];
                let (data_size, recv_from_address) = match timeout(Duration::from_secs(5), dst_udp_socket.recv_from(&mut dst_recv_buf)).await {
                    Ok(Ok((data_size, recv_from_address))) => (data_size, recv_from_address),
                    Ok(Err(e)) => {
                        error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
                        return Err(anyhow!(e));
                    },
                    Err(e) => {
                        error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of timeout");
                        return Err(anyhow!(e));
                    },
                };
                let dst_recv_buf = &dst_recv_buf[..data_size];
                debug!(
                    "Udp handler {handler_key} receive data from destination: {recv_from_address}:\n{}\n",
                    pretty_hex(&dst_recv_buf)
                );

                let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));

                let udp_data_message =
                    match PpaassMessageGenerator::generate_udp_data(user_token.clone(), payload_encryption, src_address, dst_address, dst_recv_buf.to_vec()) {
                        Ok(data_message) => data_message,
                        Err(e) => {
                            error!("Udp handler {handler_key} fail to generate udp data from [{dst_socket_addrs:?}] because of error: {e:?}");
                            return Err(anyhow!(e));
                        },
                    };

                if let Err(e) = ppaass_connection_write.send(udp_data_message).await {
                    error!("Udp handler {handler_key} fail to send udp data from [{dst_socket_addrs:?}] to agent because of error: {e:?}");
                    return Err(anyhow!(e));
                };
                Ok(())
            });
        }
    }
}
