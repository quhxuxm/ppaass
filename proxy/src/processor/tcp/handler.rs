use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::{Bytes, BytesMut};
use derive_more::{Constructor, Display};
use futures::StreamExt as FuturesStreamExt;
use futures_util::SinkExt;

use tokio::time::timeout;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use log::{debug, error};
use tokio_stream::StreamExt as TokioStreamExt;

use ppaass_common::{
    agent::PpaassAgentConnection,
    generate_uuid,
    tcp::{AgentTcpData, ProxyTcpInitResultType},
    CommonError, PpaassAgentMessage,
};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    config::PROXY_CONFIG,
    crypto::ProxyServerRsaCryptoFetcher,
    error::{NetworkError, ProxyError},
};

use super::destination::DstConnection;

#[derive(Debug, Clone, Constructor, Display)]
#[display(fmt = "[{}]#[{}]@TCP::[{}]::[{}=>{}]", connection_id, user_token, agent_address, src_address, dst_address)]
pub(crate) struct TcpHandlerKey {
    connection_id: String,
    user_token: String,
    agent_address: PpaassNetAddress,
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
}

#[derive(Constructor)]
#[non_exhaustive]
pub(crate) struct TcpHandler<'r, T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
    'r: 'static,
{
    handler_key: TcpHandlerKey,
    agent_connection: PpaassAgentConnection<'r, T, ProxyServerRsaCryptoFetcher, I>,
}

impl<'r, T, I> TcpHandler<'r, T, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
    'r: 'static,
{
    async fn init_dst_connection(handler_key: &TcpHandlerKey) -> Result<DstConnection<TcpStream>, ProxyError> {
        let dst_socket_address = handler_key.dst_address.to_socket_addrs()?.collect::<Vec<SocketAddr>>();

        let dst_tcp_stream = match timeout(
            Duration::from_secs(PROXY_CONFIG.get_dst_connect_timeout()),
            TcpStream::connect(dst_socket_address.as_slice()),
        )
        .await
        {
            Err(timeout) => {
                error!(
                    "Tcp handler {handler_key} fail connect to destination [{}] because of timeout.",
                    handler_key.dst_address
                );
                return Err(ProxyError::Io(timeout.into()));
            },
            Ok(Ok(dst_tcp_stream)) => dst_tcp_stream,
            Ok(Err(e)) => {
                error!(
                    "Tcp handler {handler_key} fail connect to dest address [{}] because of error: {e:?}",
                    handler_key.dst_address
                );
                return Err(ProxyError::Network(NetworkError::DestinationConnect(e)));
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

    pub(crate) async fn exec(self) -> Result<(), ProxyError> {
        let handler_key = self.handler_key;
        let mut agent_connection = self.agent_connection;
        let dst_relay_timeout = PROXY_CONFIG.get_dst_relay_timeout();
        let agent_relay_timeout = PROXY_CONFIG.get_agent_relay_timeout();

        let dst_connection = match Self::init_dst_connection(&handler_key).await {
            Ok(dst_connection) => dst_connection,
            Err(e) => {
                let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(Bytes::from(generate_uuid().into_bytes())));
                let tcp_init_fail = PpaassMessageGenerator::generate_proxy_tcp_init_message(
                    handler_key.to_string(),
                    handler_key.user_token.clone(),
                    handler_key.src_address.clone(),
                    handler_key.dst_address.clone(),
                    payload_encryption,
                    ProxyTcpInitResultType::Fail,
                )?;
                agent_connection.send(tcp_init_fail).await?;
                return Err(e);
            },
        };
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(Bytes::from(generate_uuid().into_bytes())));
        let tcp_init_success_message = PpaassMessageGenerator::generate_proxy_tcp_init_message(
            handler_key.to_string(),
            handler_key.user_token.clone(),
            handler_key.src_address.clone(),
            handler_key.dst_address.clone(),
            payload_encryption.clone(),
            ProxyTcpInitResultType::Success,
        )?;
        agent_connection.send(tcp_init_success_message).await?;
        debug!("Tcp handler {handler_key} create destination connection success.");
        let (mut agent_connection_write, agent_connection_read) = agent_connection.split();
        let (mut dst_connection_write, dst_connection_read) = dst_connection.split();
        let (_, _) = tokio::join!(
            TokioStreamExt::map_while(agent_connection_read.timeout(Duration::from_secs(agent_relay_timeout)), |agent_message| {
                let agent_message = agent_message.ok()?;
                let agent_message = agent_message.ok()?;
                let raw_data = Self::unwrap_to_raw_tcp_data(agent_message).ok()?;
                Some(Ok(BytesMut::from_iter(raw_data)))
            })
            .forward(&mut dst_connection_write),
            TokioStreamExt::map_while(dst_connection_read.timeout(Duration::from_secs(dst_relay_timeout)), |dst_message| {
                let dst_message = dst_message.ok()?;
                let dst_message = dst_message.ok()?;
                let tcp_data_message = PpaassMessageGenerator::generate_proxy_tcp_data_message(
                    handler_key.user_token.clone(),
                    payload_encryption.clone(),
                    handler_key.src_address.clone(),
                    handler_key.dst_address.clone(),
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
