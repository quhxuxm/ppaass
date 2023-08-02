use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use anyhow::anyhow;
use derive_more::{Constructor, Display};
use futures::SinkExt;
use ppaass_common::{generate_uuid, PpaassConnection, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress, RsaCryptoFetcher};

use log::{debug, error};
use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    time::timeout,
};

use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    config::PROXY_CONFIG,
    error::{NetworkError, ProxyError},
};

#[derive(Debug, Clone, Constructor, Display)]
#[display(fmt = "[{}]#[{}]@UDP::[{}]::[{}=>{}]", connection_id, user_token, agent_address, src_address, dst_address)]
pub(crate) struct UdpHandlerKey {
    connection_id: String,
    user_token: String,
    agent_address: PpaassNetAddress,
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
}

#[derive(Debug, Constructor)]
#[non_exhaustive]
pub(crate) struct UdpHandler<'r, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    handler_key: UdpHandlerKey,
    agent_connection: PpaassConnection<'r, T, R, I>,
}

impl<T, R, I> UdpHandler<'_, T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) async fn exec(self, udp_data: Vec<u8>) -> Result<(), ProxyError> {
        let mut agent_connection = self.agent_connection;
        let handler_key = self.handler_key;
        debug!("Udp handler {handler_key} receive agent udp data: {}", pretty_hex(&udp_data));
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
            Duration::from_secs(PROXY_CONFIG.get_dst_connect_timeout()),
            dst_udp_socket.connect(dst_socket_addrs.as_slice()),
        )
        .await
        {
            Ok(Err(e)) => {
                error!(
                    "Udp handler {handler_key} udp socket fail connect to destination address [{}] because of error: {e:?}",
                    handler_key.dst_address
                );
                return Err(ProxyError::Io(e));
            },
            Ok(Ok(())) => {
                debug!("Udp handler {handler_key} udp socket success connect to destination [{dst_socket_addrs:?}]");
            },
            Err(_) => {
                error!("Udp handler {handler_key} udp socket fail connect to destination [{dst_socket_addrs:?}] because of timeout");
                return Err(ProxyError::Network(NetworkError::Timeout(PROXY_CONFIG.get_dst_connect_timeout())));
            },
        };
        if let Err(e) = dst_udp_socket.send(&udp_data).await {
            error!("Udp handler {handler_key} fail to send data to udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
            return Err(ProxyError::Network(NetworkError::DestinationWrite(e)));
        };

        let mut udp_data = [0u8; 65535];
        let udp_data = match timeout(Duration::from_secs(PROXY_CONFIG.get_dst_udp_recv_timeout()), dst_udp_socket.recv(&mut udp_data)).await {
            Ok(Ok(0)) => {
                error!("Udp handler {handler_key} nothing to receive from udp socket [{dst_socket_addrs:?}]");
                return Err(ProxyError::Other(anyhow!("Nothing to receive from udp socket")));
            },
            Ok(Ok(data_size)) => &udp_data[..data_size],
            Ok(Err(e)) => {
                error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of error: {e:?}");
                return Err(ProxyError::Network(NetworkError::DestinationRead(e)));
            },
            Err(_) => {
                error!("Udp handler {handler_key} fail to receive data from udp socket [{dst_socket_addrs:?}] because of timeout");
                return Err(ProxyError::Network(NetworkError::Timeout(PROXY_CONFIG.get_dst_udp_recv_timeout())));
            },
        };

        debug!(
            "Udp handler {handler_key} receive data from destination: {dst_socket_addrs:?}:\n{}\n",
            pretty_hex(&udp_data)
        );
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(generate_uuid().into_bytes()));
        let udp_data_message = PpaassMessageGenerator::generate_udp_data(
            handler_key.user_token.clone(),
            payload_encryption,
            handler_key.src_address.clone(),
            handler_key.dst_address.clone(),
            udp_data.to_vec(),
        )?;
        if let Err(e) = agent_connection.send(udp_data_message).await {
            error!("Udp handler {handler_key} fail to send udp data from [{dst_socket_addrs:?}] to agent because of error: {e:?}");
            return Err(ProxyError::Network(NetworkError::AgentWrite(e)));
        };
        Ok(())
    }
}
