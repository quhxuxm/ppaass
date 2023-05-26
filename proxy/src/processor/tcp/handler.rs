use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use bytes::BytesMut;
use derive_more::{Constructor, Display};
use futures::StreamExt as FuturesStreamExt;
use futures_util::SinkExt;

use tokio::time::timeout;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use tokio_stream::StreamExt as TokioStreamExt;
use tracing::{debug, error};

use ppaass_common::{
    generate_uuid,
    tcp::{TcpData, TcpInitResponseType},
    CommonError, PpaassConnectionRead, PpaassConnectionWrite, PpaassMessage, RsaCryptoFetcher,
};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    config::ProxyServerConfig,
    error::{NetworkError, ProxyError},
};

use super::destination::{DstConnection, DstConnectionParts, DstConnectionRead, DstConnectionWrite};

#[derive(Debug, Clone, Constructor, Display)]
#[display(fmt = "[{}]#[{}]@TCP::[{}]::[{}=>{}]", connection_id, user_token, agent_address, src_address, dst_address)]
pub(crate) struct TcpHandlerKey {
    connection_id: String,
    user_token: String,
    agent_address: PpaassNetAddress,
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
}

#[derive(Debug, Constructor)]
#[non_exhaustive]
pub(crate) struct TcpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    handler_key: TcpHandlerKey,
    agent_connection_read: PpaassConnectionRead<T, R, I>,
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R, I> TcpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    async fn init_dst_connection(
        handler_key: &TcpHandlerKey, configuration: Arc<ProxyServerConfig>,
    ) -> Result<(DstConnectionRead<TcpStream>, DstConnectionWrite<TcpStream>), ProxyError> {
        let dst_socket_address = handler_key.dst_address.to_socket_addrs()?.collect::<Vec<SocketAddr>>();

        let dst_tcp_stream = match timeout(
            Duration::from_secs(configuration.get_dst_connect_timeout()),
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
        let dst_connection = DstConnection::new(dst_tcp_stream, configuration.get_dst_tcp_buffer_size());
        let DstConnectionParts {
            read: dst_tcp_read,
            write: dst_tcp_write,
            ..
        } = dst_connection.split();
        Ok((dst_tcp_read, dst_tcp_write))
    }

    fn unwrap_to_raw_tcp_data(message: PpaassMessage) -> Result<Vec<u8>, CommonError> {
        let PpaassMessage { payload, .. } = message;
        let TcpData { data, .. } = payload.try_into()?;
        Ok(data)
    }

    pub(crate) async fn exec(self) -> Result<(), ProxyError> {
        let handler_key = self.handler_key;
        let mut agent_connection_write = self.agent_connection_write;
        let agent_connection_read = self.agent_connection_read;
        let dst_relay_timeout = self.configuration.get_dst_relay_timeout();
        let agent_relay_timeout = self.configuration.get_agent_relay_timeout();

        let (dst_connection_read, dst_connection_write) = match Self::init_dst_connection(&handler_key, self.configuration).await {
            Ok(dst_read_and_write) => dst_read_and_write,
            Err(e) => {
                let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(generate_uuid().into_bytes()));
                let tcp_init_fail = PpaassMessageGenerator::generate_tcp_init_response(
                    handler_key.to_string(),
                    handler_key.user_token.clone(),
                    handler_key.src_address.clone(),
                    handler_key.dst_address.clone(),
                    payload_encryption,
                    TcpInitResponseType::Fail,
                )?;
                agent_connection_write.send(tcp_init_fail).await?;
                return Err(e);
            },
        };
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(generate_uuid().into_bytes()));
        let tcp_init_success_message = PpaassMessageGenerator::generate_tcp_init_response(
            handler_key.to_string(),
            handler_key.user_token.clone(),
            handler_key.src_address.clone(),
            handler_key.dst_address.clone(),
            payload_encryption.clone(),
            TcpInitResponseType::Success,
        )?;
        agent_connection_write.send(tcp_init_success_message).await?;
        debug!("Tcp handler {handler_key} create destination connection success.");
        {
            let handler_key = handler_key.clone();
            tokio::spawn(async move {
                if let Err(e) = TokioStreamExt::map_while(agent_connection_read.timeout(Duration::from_secs(agent_relay_timeout)), |agent_message| {
                    let agent_message = agent_message.ok()?;
                    let agent_message = agent_message.ok()?;
                    let raw_data = Self::unwrap_to_raw_tcp_data(agent_message).ok()?;
                    Some(Ok(BytesMut::from_iter(raw_data)))
                })
                .forward(dst_connection_write)
                .await
                {
                    error!("Tcp handler {handler_key} fail to relay agent data to destination because of error: {e:?}");
                };
                Ok::<_, NetworkError>(())
            });
        }
        tokio::spawn(async move {
            if let Err(e) = TokioStreamExt::map_while(dst_connection_read.timeout(Duration::from_secs(dst_relay_timeout)), |dst_message| {
                let dst_message = dst_message.ok()?;
                let dst_message = dst_message.ok()?;
                let tcp_data_message = PpaassMessageGenerator::generate_tcp_data(
                    handler_key.user_token.clone(),
                    payload_encryption.clone(),
                    handler_key.src_address.clone(),
                    handler_key.dst_address.clone(),
                    dst_message.to_vec(),
                )
                .ok()?;
                Some(Ok(tcp_data_message))
            })
            .forward(agent_connection_write)
            .await
            {
                error!("Tcp handler {handler_key} fail to relay destination data to agent because of error: {e:?}");
            }
        });

        Ok(())
    }
}
