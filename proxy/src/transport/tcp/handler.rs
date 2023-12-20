use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::{BufMut, Bytes, BytesMut};

use futures::StreamExt as FuturesStreamExt;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;

use log::{debug, error};
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use tokio::time::timeout;

use ppaass_common::{
    agent::PpaassAgentConnection,
    tcp::{AgentTcpData, ProxyTcpInitResultType},
    CommonError, PpaassAgentMessage, PpaassMessagePayloadEncryption, PpaassProxyMessage,
};
use ppaass_common::{PpaassMessageGenerator, PpaassUnifiedAddress};

use crate::{config::PROXY_CONFIG, crypto::ProxyServerRsaCryptoFetcher, error::ProxyServerError};

use super::destination::DstConnection;

#[derive(Default)]
#[non_exhaustive]
pub(crate) struct TcpHandler;

impl TcpHandler {
    async fn init_dst_connection(dst_address: &PpaassUnifiedAddress) -> Result<DstConnection, ProxyServerError> {
        let dst_socket_address = dst_address.to_socket_addrs()?.collect::<Vec<SocketAddr>>();
        let dst_tcp_stream = match timeout(
            Duration::from_secs(PROXY_CONFIG.get_dst_connect_timeout()),
            TcpStream::connect(dst_socket_address.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Initialize tcp connection to destination timeout: {dst_address}");
                return Err(ProxyServerError::Timeout(PROXY_CONFIG.get_dst_connect_timeout()));
            },
            Ok(Ok(dst_tcp_stream)) => dst_tcp_stream,
            Ok(Err(e)) => {
                error!("Initialize tcp connection to destination [{dst_address}] because of I/O error: {e:?}");
                return Err(ProxyServerError::GeneralIo(e));
            },
        };
        dst_tcp_stream.set_nodelay(true)?;
        dst_tcp_stream.set_linger(None)?;
        let dst_connection = DstConnection::new(dst_tcp_stream, PROXY_CONFIG.get_dst_tcp_buffer_size());
        Ok(dst_connection)
    }

    fn unwrap_to_raw_tcp_data(message: PpaassAgentMessage) -> Result<Bytes, CommonError> {
        let PpaassAgentMessage { payload, .. } = message;
        let AgentTcpData { data, .. } = payload.data.try_into()?;
        Ok(data)
    }

    pub(crate) async fn exec(
        mut agent_connection: PpaassAgentConnection<ProxyServerRsaCryptoFetcher>, agent_tcp_init_message_id: String, user_token: String,
        src_address: PpaassUnifiedAddress, dst_address: PpaassUnifiedAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<(), ProxyServerError> {
        let dst_connection = match Self::init_dst_connection(&dst_address).await {
            Ok(dst_connection) => dst_connection,
            Err(e) => {
                let tcp_init_fail = PpaassMessageGenerator::generate_proxy_tcp_init_message(
                    agent_tcp_init_message_id,
                    user_token,
                    src_address,
                    dst_address,
                    payload_encryption,
                    ProxyTcpInitResultType::ConnectToDstFail,
                )?;
                agent_connection.send(tcp_init_fail).await?;
                return Err(e);
            },
        };
        let tcp_init_success_message = PpaassMessageGenerator::generate_proxy_tcp_init_message(
            agent_tcp_init_message_id,
            user_token.clone(),
            src_address.clone(),
            dst_address.clone(),
            payload_encryption.clone(),
            ProxyTcpInitResultType::Success,
        )?;
        agent_connection.send(tcp_init_success_message).await?;
        let (agent_connection_write, agent_connection_read) = agent_connection.split();
        let (dst_connection_write, dst_connection_read) = dst_connection.split();

        let (dst_inbound_tx, dst_inbound_rx) = unbounded_channel::<Bytes>();
        let (dst_outbound_tx, dst_outbound_rx) = unbounded_channel::<Bytes>();

        tokio::spawn(Self::read_agent_connection_to_dst_outbound_tx(agent_connection_read, dst_outbound_tx));
        tokio::spawn(Self::read_dst_connection_to_dst_inbound_tx(dst_connection_read, dst_inbound_tx));
        tokio::spawn(Self::write_dst_inbound_rx_to_agent_connection(
            dst_inbound_rx,
            agent_connection_write,
            user_token,
            payload_encryption,
            src_address,
            dst_address,
        ));
        tokio::spawn(Self::write_dst_outbound_rx_to_dst_connection(dst_outbound_rx, dst_connection_write));
        Ok(())
    }

    async fn read_agent_connection_to_dst_outbound_tx(
        mut agent_connection_read: SplitStream<PpaassAgentConnection<ProxyServerRsaCryptoFetcher>>, dst_outbound_tx: UnboundedSender<Bytes>,
    ) {
        loop {
            let agent_message = match timeout(Duration::from_secs(PROXY_CONFIG.get_agent_relay_timeout()), agent_connection_read.next()).await {
                Ok(Some(Ok(agent_message))) => agent_message,
                Ok(Some(Err(e))) => {
                    error!("Fail to forward agent message to destination because of error happen when read agent connection: {e:?}");
                    return;
                },
                Ok(None) => {
                    debug!("Read all data from agent connection");
                    return;
                },
                Err(_) => {
                    error!("Fail to forward agent message to destination because of read agent connection timeout.");
                    return;
                },
            };
            let raw_data = match Self::unwrap_to_raw_tcp_data(agent_message) {
                Ok(raw_data) => raw_data,
                Err(e) => {
                    error!("Fail to unwrap agent message because of error: {e:?}");
                    return;
                },
            };
            if let Err(e) = dst_outbound_tx.send(raw_data) {
                error!("Fail to send agent message to destination outbound channel because of error: {e:?}");
                return;
            };
        }
    }

    async fn read_dst_connection_to_dst_inbound_tx(mut dst_connection_read: SplitStream<DstConnection>, dst_inbound_tx: UnboundedSender<Bytes>) {
        loop {
            let dst_message = match timeout(Duration::from_secs(PROXY_CONFIG.get_dst_relay_timeout()), dst_connection_read.next()).await {
                Ok(Some(Ok(dst_message))) => dst_message,
                Ok(Some(Err(e))) => {
                    error!("Fail to forward destination message to agent because of error happen when read destination connection: {e:?}");
                    return;
                },
                Ok(None) => {
                    debug!("Read all data from destination connection");
                    return;
                },
                Err(_) => {
                    error!("Fail to forward destination message to agent because of read agent connection timeout.");
                    return;
                },
            };
            if let Err(e) = dst_inbound_tx.send(dst_message.freeze()) {
                error!("Fail to send destination message to destination inbound channel because of error: {e:?}");
                return;
            };
        }
    }
    async fn write_dst_inbound_rx_to_agent_connection(
        mut dst_inbound_rx: UnboundedReceiver<Bytes>,
        mut agent_connection_write: SplitSink<PpaassAgentConnection<ProxyServerRsaCryptoFetcher>, PpaassProxyMessage>, user_token: String,
        payload_encryption: PpaassMessagePayloadEncryption, src_address: PpaassUnifiedAddress, dst_address: PpaassUnifiedAddress,
    ) {
        loop {
            let mut all_inbound_data = Vec::with_capacity(65536);
            match dst_inbound_rx.try_recv() {
                Ok(inbound_data) => {
                    all_inbound_data.push(inbound_data);
                },
                Err(TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                    continue;
                },
                Err(TryRecvError::Disconnected) => {
                    return;
                },
            }
            let mut data = BytesMut::new();
            all_inbound_data.iter().for_each(|item| {
                data.put_slice(item);
            });

            let tcp_data_message = match PpaassMessageGenerator::generate_proxy_tcp_data_message(
                user_token.clone(),
                payload_encryption.clone(),
                src_address.clone(),
                dst_address.clone(),
                data.freeze(),
            ) {
                Err(e) => {
                    error!("Fail to create proxy message with destination data because of error: {e:?}");
                    return;
                },
                Ok(tcp_data_message) => tcp_data_message,
            };
            if let Err(e) = agent_connection_write.send(tcp_data_message).await {
                error!("Fail to send proxy message to agent because of error: {e:?}");
                return;
            };
        }
    }
    async fn write_dst_outbound_rx_to_dst_connection(
        mut dst_outbound_rx: UnboundedReceiver<Bytes>, mut dst_connection_write: SplitSink<DstConnection, BytesMut>,
    ) {
        loop {
            let mut all_outbound_data = Vec::with_capacity(65536);
            match dst_outbound_rx.try_recv() {
                Ok(outbound_data) => {
                    all_outbound_data.push(outbound_data);
                },
                Err(TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                    continue;
                },
                Err(TryRecvError::Disconnected) => {
                    return;
                },
            }
            let mut data = BytesMut::new();
            all_outbound_data.iter().for_each(|item| {
                data.put_slice(item);
            });
            if let Err(e) = dst_connection_write.send(data).await {
                error!("Fail to send agent message to destination because of error: {e:?}");
                return;
            };
        }
    }
}