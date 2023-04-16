use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use futures::SinkExt;
use ppaass_common::{
    generate_uuid,
    udp::{UdpData, UdpDataParts},
    PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress, RsaCryptoFetcher,
};
use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    time::timeout,
};
use tracing::{debug, error, info};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig};

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
    pub(crate) async fn build(self, configuration: Arc<ProxyServerConfig>) -> Result<UdpHandler<T, R, I>> {
        let ppaass_connection_id = self.ppaass_connection_id.context("Agent connection id not assigned for udp handler builder")?;
        let agent_address = self.agent_address.context("Agent address not assigned for udp handler builder")?;
        let user_token = self.user_token.context("User token not assigned for udp handler builder")?;
        let handler_key = Self::generate_handler_key(&user_token, &agent_address, &ppaass_connection_id);
        let ppaass_connection_write = self
            .ppaass_connection_write
            .context("Agent message framed write not assigned for udp handler builder")?;

        Ok(UdpHandler {
            handler_key,
            ppaass_connection_write,
            user_token,
            configuration,
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
    ppaass_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: String,
    user_token: String,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R, I> UdpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) async fn exec(self, udp_data: UdpData) -> Result<()> {
        let mut ppaass_connection_write = self.ppaass_connection_write;

        let user_token = self.user_token;
        let handler_key = self.handler_key;

        let dst_udp_socket = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(dst_udp_socket) => dst_udp_socket,
            Err(e) => {
                if let Err(e) = ppaass_connection_write.close().await {
                    error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                }
                return Err(anyhow!(e));
            },
        };

        let UdpDataParts {
            src_address,
            dst_address,
            raw_data,
        } = udp_data.split();

        let dst_socket_addrs = match dst_address.to_socket_addrs() {
            Ok(v) => v,
            Err(e) => {
                error!("Udp handler {handler_key} fail to convert destination address [{dst_address}] because of error: {e:?}");
                if let Err(e) = ppaass_connection_write.close().await {
                    error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                }
                return Err(anyhow!(e));
            },
        };
        info!(
            "Udp handler {handler_key} receive agent udp data from [{dst_address}]:\n{}\n",
            pretty_hex(&raw_data)
        );
        let dst_socket_addrs = dst_socket_addrs.collect::<Vec<SocketAddr>>();
        dst_udp_socket.connect(dst_socket_addrs.as_slice()).await?;
        if let Err(e) = dst_udp_socket.send(&raw_data).await {
            error!("Udp handler {handler_key} fail to send data to udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
            if let Err(e) = ppaass_connection_write.close().await {
                error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
            }
            return Err(anyhow!(e));
        };
        let mut dst_recv_buf = Vec::new();
        loop {
            let mut buf = [0u8; 65535];
            let data_size = match timeout(
                Duration::from_secs(self.configuration.get_dst_udp_recv_timeout()),
                dst_udp_socket.recv(&mut buf),
            )
            .await
            {
                Ok(Ok(data_size)) => data_size,
                Ok(Err(e)) => {
                    error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
                    if let Err(e) = ppaass_connection_write.close().await {
                        error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(e));
                },
                Err(e) => {
                    error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of timeout");
                    if let Err(e) = ppaass_connection_write.close().await {
                        error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(e));
                },
            };
            if data_size == 0 {
                break;
            }
            let buf = &buf[0..data_size];
            dst_recv_buf.extend_from_slice(buf);
        }
        if dst_recv_buf.is_empty() {
            debug!("Udp handler {handler_key} nothing received from destination: {dst_socket_addrs:?}");
            return Ok(());
        }
        debug!(
            "Udp handler {handler_key} receive data from destination: {dst_socket_addrs:?}:\n{}\n",
            pretty_hex(&dst_recv_buf)
        );

        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));

        let udp_data_message =
            match PpaassMessageGenerator::generate_udp_data(user_token.clone(), payload_encryption, src_address, dst_address, dst_recv_buf.to_vec()) {
                Ok(data_message) => data_message,
                Err(e) => {
                    error!("Udp handler {handler_key} fail to generate udp data from [{dst_socket_addrs:?}] because of error: {e:?}");
                    if let Err(e) = ppaass_connection_write.close().await {
                        error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(
                        "Udp handler {handler_key} fail to generate udp data from [{dst_socket_addrs:?}] because of error: {e:?}"
                    ));
                },
            };

        if let Err(e) = ppaass_connection_write.send(udp_data_message).await {
            error!("Udp handler {handler_key} fail to send udp data from [{dst_socket_addrs:?}] to agent because of error: {e:?}");
            if let Err(e) = ppaass_connection_write.close().await {
                error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
            }
            return Err(anyhow!(
                "Udp handler {handler_key} fail to send udp data from [{dst_socket_addrs:?}] to agent because of error: {e:?}"
            ));
        };
        if let Err(e) = ppaass_connection_write.close().await {
            error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
        }
        Ok(())
    }
}
