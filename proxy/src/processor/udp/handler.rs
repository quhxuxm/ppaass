use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use anyhow::anyhow;
use bytes::{BufMut, Bytes, BytesMut};
use futures::SinkExt;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    time::timeout,
};

use ppaass_common::{agent::PpaassAgentConnection, generate_uuid, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    config::PROXY_CONFIG,
    crypto::ProxyServerRsaCryptoFetcher,
    error::{NetworkError, ProxyError},
};

const MAX_UDP_PACKET_SIZE: usize = 65535;

pub(crate) struct UdpHandler;

impl UdpHandler {
    pub(crate) async fn exec<T, I, U>(
        mut agent_connection: PpaassAgentConnection<'_, T, ProxyServerRsaCryptoFetcher, I>, user_token: U, src_address: PpaassNetAddress,
        dst_address: PpaassNetAddress, udp_data: Bytes,
    ) -> Result<(), ProxyError>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
        U: ToString + AsRef<str> + Clone,
    {
        let dst_udp_socket = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(dst_udp_socket) => dst_udp_socket,
            Err(e) => {
                return Err(ProxyError::Io(e));
            },
        };
        let dst_socket_addrs = match dst_address.to_socket_addrs() {
            Ok(dst_socket_addrs) => dst_socket_addrs,
            Err(e) => {
                return Err(ProxyError::Io(e));
            },
        };

        let dst_socket_addrs = dst_socket_addrs.collect::<Vec<SocketAddr>>();
        match timeout(
            Duration::from_secs(PROXY_CONFIG.get_dst_udp_connect_timeout()),
            dst_udp_socket.connect(dst_socket_addrs.as_slice()),
        )
        .await
        {
            Ok(Err(e)) => {
                return Err(ProxyError::Io(e));
            },
            Ok(Ok(())) => {},
            Err(_) => {
                return Err(ProxyError::Network(NetworkError::Timeout(PROXY_CONFIG.get_dst_connect_timeout())));
            },
        };
        if let Err(e) = dst_udp_socket.send(&udp_data).await {
            return Err(ProxyError::Network(NetworkError::DestinationWrite(e)));
        };

        let mut udp_data = BytesMut::new();
        loop {
            let mut udp_recv_buf = [0u8; MAX_UDP_PACKET_SIZE];
            let udp_recv_buf = match timeout(
                Duration::from_secs(PROXY_CONFIG.get_dst_udp_recv_timeout()),
                dst_udp_socket.recv(&mut udp_recv_buf),
            )
            .await
            {
                Ok(Ok(0)) => {
                    return Err(ProxyError::Other(anyhow!("Nothing to receive from udp socket")));
                },
                Ok(Ok(size)) => &udp_recv_buf[..size],
                Ok(Err(e)) => {
                    return Err(ProxyError::Network(NetworkError::DestinationRead(e)));
                },
                Err(_) => {
                    return Err(ProxyError::Network(NetworkError::Timeout(PROXY_CONFIG.get_dst_udp_recv_timeout())));
                },
            };
            udp_data.put(udp_recv_buf);
            if udp_recv_buf.len() < MAX_UDP_PACKET_SIZE {
                break;
            }
        }

        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(user_token.as_ref(), Some(Bytes::from(generate_uuid().into_bytes())));
        let udp_data_message = PpaassMessageGenerator::generate_proxy_udp_data_message(
            user_token.clone(),
            payload_encryption,
            src_address.clone(),
            dst_address.clone(),
            udp_data.freeze(),
        )?;
        if let Err(e) = agent_connection.send(udp_data_message).await {
            return Err(ProxyError::Network(NetworkError::AgentWrite(e)));
        };
        if let Err(e) = agent_connection.close().await {
            return Err(ProxyError::Network(NetworkError::AgentClose(e)));
        };
        Ok(())
    }
}
