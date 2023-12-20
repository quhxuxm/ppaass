use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::{BufMut, Bytes, BytesMut};
use futures::SinkExt;
use log::{debug, error};
use tokio::{net::UdpSocket, time::timeout};

use ppaass_common::{agent::PpaassAgentConnection, PpaassMessageGenerator, PpaassMessagePayloadEncryption, PpaassUnifiedAddress};

use crate::{config::PROXY_CONFIG, crypto::ProxyServerRsaCryptoFetcher, error::ProxyServerError};

const MAX_UDP_PACKET_SIZE: usize = 65535;
const LOCAL_UDP_BIND_ADDR: &str = "0.0.0.0:0";

pub(crate) struct UdpHandler;

impl UdpHandler {
    pub(crate) async fn exec(
        mut agent_connection: PpaassAgentConnection<ProxyServerRsaCryptoFetcher>, user_token: String, src_address: PpaassUnifiedAddress,
        dst_address: PpaassUnifiedAddress, udp_data: Bytes, payload_encryption: PpaassMessagePayloadEncryption, need_response: bool,
    ) -> Result<(), ProxyServerError> {
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
        if !need_response {
            return Ok(());
        }
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
                    debug!("Receive udp data from destination timeout: {dst_address}");
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