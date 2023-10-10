use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::{Bytes, BytesMut};

use futures::StreamExt as FuturesStreamExt;
use futures_util::SinkExt;

use log::error;
use tokio::time::timeout;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use tokio_stream::StreamExt as TokioStreamExt;

use ppaass_common::{
    agent::PpaassAgentConnection,
    generate_uuid,
    tcp::{AgentTcpData, ProxyTcpInitResultType},
    CommonError, PpaassAgentMessage,
};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::PROXY_CONFIG, crypto::ProxyServerRsaCryptoFetcher, error::ProxyError};

use super::destination::DstConnection;

#[derive(Default)]
#[non_exhaustive]
pub(crate) struct TcpHandler;

impl TcpHandler {
    async fn init_dst_connection(dst_address: &PpaassNetAddress) -> Result<DstConnection<TcpStream>, ProxyError> {
        let dst_socket_address = dst_address.to_socket_addrs()?.collect::<Vec<SocketAddr>>();
        let dst_tcp_stream = match timeout(
            Duration::from_secs(PROXY_CONFIG.get_dst_connect_timeout()),
            TcpStream::connect(dst_socket_address.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Initialize tcp connection to destination timeout: {dst_address}");
                return Err(ProxyError::Timeout(PROXY_CONFIG.get_dst_connect_timeout()).into());
            },
            Ok(dst_tcp_stream) => dst_tcp_stream.map_err(ProxyError::DestinationConnect)?,
        };
        dst_tcp_stream.set_nodelay(true).map_err(ProxyError::DestinationConnectionConfig)?;
        dst_tcp_stream.set_linger(None).map_err(ProxyError::DestinationConnectionConfig)?;
        let dst_connection = DstConnection::new(dst_tcp_stream, PROXY_CONFIG.get_dst_tcp_buffer_size());
        Ok(dst_connection)
    }

    fn unwrap_to_raw_tcp_data(message: PpaassAgentMessage) -> Result<Bytes, CommonError> {
        let PpaassAgentMessage { payload, .. } = message;
        let AgentTcpData { data, .. } = payload.data.try_into()?;
        Ok(data)
    }

    pub(crate) async fn exec<'r, T, I, U>(
        mut agent_connection: PpaassAgentConnection<'r, T, ProxyServerRsaCryptoFetcher, I>, user_token: U, src_address: PpaassNetAddress,
        dst_address: PpaassNetAddress,
    ) -> Result<(), ProxyError>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
        I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
        U: ToString + AsRef<str> + Clone,
        'r: 'static,
    {
        let dst_relay_timeout = PROXY_CONFIG.get_dst_relay_timeout();
        let agent_relay_timeout = PROXY_CONFIG.get_agent_relay_timeout();

        let dst_connection = match Self::init_dst_connection(&dst_address).await {
            Ok(dst_connection) => dst_connection,
            Err(e) => {
                let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(Bytes::from(generate_uuid().into_bytes())));
                let tcp_init_fail = PpaassMessageGenerator::generate_proxy_tcp_init_message(
                    generate_uuid(),
                    user_token,
                    src_address,
                    dst_address,
                    payload_encryption,
                    ProxyTcpInitResultType::Fail,
                )
                .map_err(ProxyError::Common)?;
                agent_connection.send(tcp_init_fail).await.map_err(ProxyError::AgentWrite)?;
                return Err(e);
            },
        };
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(user_token.as_ref(), Some(Bytes::from(generate_uuid().into_bytes())));
        let tcp_init_success_message = PpaassMessageGenerator::generate_proxy_tcp_init_message(
            generate_uuid(),
            user_token.clone(),
            src_address.clone(),
            dst_address.clone(),
            payload_encryption.clone(),
            ProxyTcpInitResultType::Success,
        )
        .map_err(ProxyError::Common)?;
        agent_connection.send(tcp_init_success_message).await.map_err(ProxyError::AgentWrite)?;
        let (mut agent_connection_write, agent_connection_read) = agent_connection.split();
        let (mut dst_connection_write, dst_connection_read) = dst_connection.split();
        let (_, _) = tokio::join!(
            // Forward agent data to proxy
            TokioStreamExt::map_while(agent_connection_read.timeout(Duration::from_secs(agent_relay_timeout)), |agent_message| {
                let agent_message = agent_message.ok()?;
                let agent_message = agent_message.ok()?;
                let raw_data = Self::unwrap_to_raw_tcp_data(agent_message).ok()?;
                Some(Ok(BytesMut::from_iter(raw_data)))
            })
            .forward(&mut dst_connection_write),
            // Forward proxy data to agent
            TokioStreamExt::map_while(dst_connection_read.timeout(Duration::from_secs(dst_relay_timeout)), |dst_message| {
                let dst_message = dst_message.ok()?;
                let dst_message = dst_message.ok()?;
                let tcp_data_message = PpaassMessageGenerator::generate_proxy_tcp_data_message(
                    user_token.clone(),
                    payload_encryption.clone(),
                    src_address.clone(),
                    dst_address.clone(),
                    dst_message.freeze(),
                )
                .ok()?;
                Some(Ok(tcp_data_message))
            })
            .forward(&mut agent_connection_write)
        );
        Ok(())
    }
}
