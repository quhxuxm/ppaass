use std::{
    fmt::{Debug, Display},
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::{BufMut, Bytes, BytesMut};
use futures::SinkExt;
use log::error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::UdpSocket,
    time::timeout,
};

use ppaass_common::{agent::PpaassAgentConnection, generate_uuid, PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::PROXY_CONFIG, crypto::ProxyServerRsaCryptoFetcher, error::ProxyServerError};

const MAX_UDP_PACKET_SIZE: usize = 65535;
const LOCAL_UDP_BIND_ADDR: &str = "0.0.0.0:0";

pub(crate) struct UdpHandler;

impl UdpHandler {
    pub(crate) async fn exec<T, I, U>(
        mut agent_connection: PpaassAgentConnection<'_, T, ProxyServerRsaCryptoFetcher, I>, user_token: U, src_address: PpaassNetAddress,
        dst_address: PpaassNetAddress, udp_data: Bytes,
    ) -> Result<(), ProxyServerError>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
        U: ToString + AsRef<str> + Clone,
    {
        let dst_udp_socket = UdpSocket::bind(LOCAL_UDP_BIND_ADDR).await?;
        let dst_socket_addrs = dst_address.to_socket_addrs()?;

        let dst_socket_addrs = dst_socket_addrs.collect::<Vec<SocketAddr>>();
        match timeout(
            Duration::from_secs(PROXY_CONFIG.get_dst_udp_connect_timeout()),
            dst_udp_socket.connect(dst_socket_addrs.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Initialize udp socket to destination timeout: {dst_address}");
                return Err(ProxyServerError::Timeout(PROXY_CONFIG.get_dst_connect_timeout()));
            },
            Ok(result) => result?,
        };
        dst_udp_socket.send(&udp_data).await?;
        let mut udp_data = BytesMut::new();
        loop {
            let mut udp_recv_buf = [0u8; MAX_UDP_PACKET_SIZE];
            let (udp_recv_buf, size) = match timeout(
                Duration::from_secs(PROXY_CONFIG.get_dst_udp_recv_timeout()),
                dst_udp_socket.recv(&mut udp_recv_buf),
            )
            .await
            {
                Err(_) => {
                    error!("Receive udp data from destination timeout: {dst_address}");
                    return Err(ProxyServerError::Timeout(PROXY_CONFIG.get_dst_udp_recv_timeout()));
                },

                Ok(Ok(0)) => {
                    return Ok(());
                },
                Ok(size) => {
                    let size = size?;
                    (&udp_recv_buf[..size], size)
                },
            };
            udp_data.put(udp_recv_buf);
            if size < MAX_UDP_PACKET_SIZE {
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
        agent_connection.send(udp_data_message).await?;
        agent_connection.close().await?;
        Ok(())
    }
}
