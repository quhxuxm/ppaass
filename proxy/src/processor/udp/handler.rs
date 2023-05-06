use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use futures::SinkExt;
use ppaass_common::{generate_uuid, PpaassConnectionWrite, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress, RsaCryptoFetcher};
use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    time::timeout,
};
use tracing::{debug, error};

use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    config::ProxyServerConfig,
    error::{NetworkError, ProxyError},
};

#[derive(Debug, Clone)]
pub(crate) struct UdpHandlerKey {
    pub connection_id: String,
    pub user_token: String,
    pub agent_address: PpaassNetAddress,
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
}

impl Display for UdpHandlerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}]#[{}]@UDP::[{}]::[{}=>{}]",
            self.connection_id, self.user_token, self.agent_address, self.src_address, self.dst_address
        )
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
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: UdpHandlerKey,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R, I> UdpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn new(handler_key: UdpHandlerKey, agent_connection_write: PpaassConnectionWrite<T, R, I>, configuration: Arc<ProxyServerConfig>) -> Self {
        Self {
            agent_connection_write,
            handler_key,
            configuration,
        }
    }
    pub(crate) async fn exec(self, udp_data: Vec<u8>) -> Result<(), ProxyError> {
        let mut agent_connection_write = self.agent_connection_write;
        let handler_key = self.handler_key;
        let dst_udp_socket = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(dst_udp_socket) => dst_udp_socket,
            Err(e) => {
                error!("Udp handler {handler_key} fail to bind udp socket because of error: {e:?}");
                return Err(ProxyError::Io(e));
            },
        };
        let dst_socket_addrs = match handler_key.dst_address.to_socket_addrs() {
            Ok(dst_socket_addrs) => dst_socket_addrs,
            Err(e) => {
                error!(
                    "Udp handler {handler_key} fail to convert destination address [{}] because of error: {e:?}",
                    handler_key.dst_address
                );
                return Err(ProxyError::Io(e));
            },
        };
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
                return Err(ProxyError::Network(NetworkError::DestinationConnect(e)));
            },
            Err(_) => {
                error!("Udp handler {handler_key} fail to connect destination udp socket because of timeout.");
                return Err(ProxyError::Network(NetworkError::Timeout(self.configuration.get_dst_udp_connect_timeout())));
            },
        };
        if let Err(e) = dst_udp_socket.send(&udp_data).await {
            error!("Udp handler {handler_key} fail to send data to udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
            return Err(ProxyError::Network(NetworkError::DestinationWrite(e)));
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
                    return Err(ProxyError::Network(NetworkError::DestinationRead(e)));
                },
                Err(_) => {
                    error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of timeout");
                    return Err(ProxyError::Network(NetworkError::Timeout(self.configuration.get_dst_udp_recv_timeout())));
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
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(generate_uuid().into_bytes()));
        let udp_data_message = PpaassMessageGenerator::generate_udp_data(
            handler_key.user_token.clone(),
            payload_encryption,
            handler_key.src_address.clone(),
            handler_key.dst_address.clone(),
            dst_recv_buf.to_vec(),
        )?;
        if let Err(e) = agent_connection_write.send(udp_data_message).await {
            error!("Udp handler {handler_key} fail to send udp data from [{dst_socket_addrs:?}] to agent because of error: {e:?}");
            return Err(ProxyError::Network(NetworkError::AgentWrite(e)));
        };
        Ok(())
    }
}
