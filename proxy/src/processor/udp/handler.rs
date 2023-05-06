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

use anyhow::{anyhow, Result};

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct UdpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
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
    fn generate_handler_key(user_token: &str, agent_address: &PpaassNetAddress, ppaass_connection_id: &str) -> String {
        format!("[{ppaass_connection_id}]#[{user_token}]@UDP::[{agent_address}]")
    }
    pub(crate) fn new(
        connection_id: String, user_token: String, agent_address: PpaassNetAddress, agent_connection_write: PpaassConnectionWrite<T, R, I>,
        configuration: Arc<ProxyServerConfig>,
    ) -> Self {
        let handler_key = Self::generate_handler_key(&user_token, &agent_address, &connection_id);
        Self {
            agent_connection_write,
            handler_key,
            user_token,
            configuration,
        }
    }
    pub(crate) async fn exec(self, udp_data: UdpData) -> Result<()> {
        let mut agent_connection_write = self.agent_connection_write;

        let user_token = self.user_token;
        let handler_key = self.handler_key;

        let dst_udp_socket = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(dst_udp_socket) => dst_udp_socket,
            Err(e) => {
                if let Err(e) = agent_connection_write.close().await {
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
            Ok(dst_socket_addrs) => dst_socket_addrs,
            Err(e) => {
                error!("Udp handler {handler_key} fail to convert destination address [{dst_address}] because of error: {e:?}");
                if let Err(e) = agent_connection_write.close().await {
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
        match timeout(
            Duration::from_secs(self.configuration.get_dst_udp_connect_timeout()),
            dst_udp_socket.connect(dst_socket_addrs.as_slice()),
        )
        .await
        {
            Ok(Ok(())) => {
                debug!("Udp handler {handler_key} connect to destination udp socket success.")
            },
            Ok(Err(e)) => {
                error!("Udp handler {handler_key} fail to connect destination udp socket because of error: {e:?}");
                return Err(anyhow!(e));
            },
            Err(_) => {
                error!("Udp handler {handler_key} fail to connect destination udp socket because of timeout.");
                return Err(anyhow!("Udp handler {handler_key} fail to connect destination udp socket because of timeout."));
            },
        };
        if let Err(e) = dst_udp_socket.send(&raw_data).await {
            error!("Udp handler {handler_key} fail to send data to udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
            if let Err(e) = agent_connection_write.close().await {
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
                Ok(Ok(0)) => {
                    debug!("Complete read from destination udp socket.");
                    break;
                },
                Ok(Ok(data_size)) => data_size,
                Ok(Err(e)) => {
                    error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
                    if let Err(e) = agent_connection_write.close().await {
                        error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(e));
                },
                Err(e) => {
                    error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of timeout");
                    if let Err(e) = agent_connection_write.close().await {
                        error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(e));
                },
            };
            let buf = &buf[0..data_size];
            dst_recv_buf.extend_from_slice(buf);
            if data_size < 65535 {
                break;
            }
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
                    if let Err(e) = agent_connection_write.close().await {
                        error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
                    }
                    return Err(anyhow!(
                        "Udp handler {handler_key} fail to generate udp data from [{dst_socket_addrs:?}] because of error: {e:?}"
                    ));
                },
            };

        if let Err(e) = agent_connection_write.send(udp_data_message).await {
            error!("Udp handler {handler_key} fail to send udp data from [{dst_socket_addrs:?}] to agent because of error: {e:?}");
            if let Err(e) = agent_connection_write.close().await {
                error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
            }
            return Err(anyhow!(
                "Udp handler {handler_key} fail to send udp data from [{dst_socket_addrs:?}] to agent because of error: {e:?}"
            ));
        };
        if let Err(e) = agent_connection_write.close().await {
            error!("Udp handler {handler_key} fail to close tcp connection because of error: {e:?}");
        }
        Ok(())
    }
}
