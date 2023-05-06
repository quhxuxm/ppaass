use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use bytes::BytesMut;
use futures::StreamExt;
use futures_util::SinkExt;

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};
use tokio::{sync::mpsc::channel, time::timeout};
use tracing::{debug, error};

use ppaass_common::{
    generate_uuid,
    tcp::{TcpData, TcpDataParts, TcpInitResponseType},
    CommonError, PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageParts, PpaassMessagePayloadEncryption, RsaCryptoFetcher,
};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    config::ProxyServerConfig,
    error::{ConnectionStateError, NetworkError, ProxyError},
};

use super::destination::{DstConnection, DstConnectionParts, DstConnectionRead, DstConnectionWrite};

#[derive(Debug, Clone)]
pub(crate) struct TcpHandlerKey {
    pub connection_id: String,
    pub user_token: String,
    pub agent_address: PpaassNetAddress,
    pub src_address: PpaassNetAddress,
    pub dst_address: PpaassNetAddress,
}

impl Display for TcpHandlerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}]#[{}]@TCP::[{}]::[{}=>{}]",
            self.connection_id, self.user_token, self.agent_address, self.src_address, self.dst_address
        )
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct TcpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    agent_connection_read: PpaassConnectionRead<T, R, I>,
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: TcpHandlerKey,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R, I> TcpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn new(
        handler_key: TcpHandlerKey, agent_connection_read: PpaassConnectionRead<T, R, I>, agent_connection_write: PpaassConnectionWrite<T, R, I>,
        configuration: Arc<ProxyServerConfig>,
    ) -> Self {
        Self {
            handler_key,
            agent_connection_read,
            agent_connection_write,
            configuration,
        }
    }

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

    async fn write_dst_data_to_agent(
        handler_key: &TcpHandlerKey, payload_encryption: PpaassMessagePayloadEncryption, mut agent_tcp_write: PpaassConnectionWrite<T, R, I>,
        mut dst_to_agent_receiver: Receiver<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let dst_data = match dst_to_agent_receiver.recv().await {
                Some(dst_data) => dst_data,
                None => {
                    debug!("Tcp handler {handler_key} complete to read destination data.");
                    return Ok(());
                },
            };
            let tcp_data_message = PpaassMessageGenerator::generate_tcp_data(
                handler_key.user_token.clone(),
                payload_encryption.clone(),
                handler_key.src_address.clone(),
                handler_key.dst_address.clone(),
                dst_data.to_vec(),
            )?;
            agent_tcp_write
                .send(tcp_data_message)
                .await
                .map_err(|e| ProxyError::Network(NetworkError::AgentWrite(e)))?;
        }
    }

    async fn write_agent_data_to_dst(
        handler_key: &TcpHandlerKey, mut dst_tcp_write: DstConnectionWrite<TcpStream>, mut agent_to_dst_receiver: Receiver<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let agent_data = match agent_to_dst_receiver.recv().await {
                Some(agent_data) => agent_data,
                None => {
                    debug!("Tcp handler {handler_key} complete to read agent data.");
                    return Ok(());
                },
            };
            dst_tcp_write.send(agent_data).await.map_err(ProxyError::Network)?;
        }
    }

    async fn read_dst_data(
        handler_key: &TcpHandlerKey, mut dst_tcp_read: DstConnectionRead<TcpStream>, dst_to_agent_sender: Sender<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let dst_message = match dst_tcp_read.next().await {
                Some(Ok(dst_message)) => dst_message,
                Some(Err(e)) => {
                    error!("Tcp handler {handler_key} fail to read destination data because of error: {e:?}");
                    return Err(ProxyError::Network(e));
                },
                None => {
                    debug!("Tcp handler {handler_key} complete to read destination data.");
                    return Ok(());
                },
            };
            dst_to_agent_sender.send(dst_message).await.map_err(|e| ProxyError::Other(anyhow!(e)))?;
        }
    }

    async fn read_agent_data(
        handler_key: &TcpHandlerKey, mut agent_tcp_read: PpaassConnectionRead<T, R, I>, agent_to_dst_sender: Sender<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let agent_message = match agent_tcp_read.next().await {
                Some(Ok(agent_message)) => agent_message,
                Some(Err(e)) => {
                    error!("Tcp handler {handler_key} fail to read agent data because of error: {e:?}");
                    return Err(ProxyError::Network(NetworkError::AgentRead(e)));
                },
                None => {
                    debug!("Tcp handler {handler_key} complete to read agent data.");
                    return Ok(());
                },
            };
            let PpaassMessageParts { payload: agent_message, .. } = agent_message.split();
            let tcp_data: TcpData = agent_message.try_into().map_err(|e| match e {
                CommonError::Decoder(e) => ProxyError::ConnectionState(ConnectionStateError::TcpData(e)),
                e => ProxyError::Common(e),
            })?;
            let TcpDataParts { raw_data, .. } = tcp_data.split();
            let raw_data = BytesMut::from_iter(raw_data);
            agent_to_dst_sender.send(raw_data).await.map_err(|e| ProxyError::Other(anyhow!(e)))?;
        }
    }

    pub(crate) async fn exec(self) -> Result<(), ProxyError> {
        let handler_key = self.handler_key;
        let mut agent_connection_write = self.agent_connection_write;
        let agent_connection_read = self.agent_connection_read;

        let (dst_tcp_read, dst_tcp_write) = match Self::init_dst_connection(&handler_key, self.configuration).await {
            Ok(dst_read_and_write) => dst_read_and_write,
            Err(e) => {
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(generate_uuid().into_bytes()));
                let tcp_init_fail = PpaassMessageGenerator::generate_tcp_init_response(
                    handler_key.to_string(),
                    handler_key.user_token.clone(),
                    handler_key.src_address.clone(),
                    handler_key.dst_address.clone(),
                    payload_encryption_token,
                    TcpInitResponseType::Fail,
                )?;
                agent_connection_write.send(tcp_init_fail).await?;
                return Err(e);
            },
        };
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&handler_key.user_token, Some(generate_uuid().into_bytes()));
        let tcp_init_success_message = PpaassMessageGenerator::generate_tcp_init_response(
            handler_key.to_string(),
            handler_key.user_token.clone(),
            handler_key.src_address.clone(),
            handler_key.dst_address.clone(),
            payload_encryption_token.clone(),
            TcpInitResponseType::Success,
        )?;
        agent_connection_write.send(tcp_init_success_message).await?;
        debug!("Tcp handler {handler_key} create destination connection success.");
        let (agent_to_dst_sender, agent_to_dst_receiver) = channel(1024);
        let (dst_to_agent_sender, dst_to_agent_receiver) = channel(1024);
        {
            let handler_key = handler_key.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::read_agent_data(&handler_key, agent_connection_read, agent_to_dst_sender).await {
                    error!("Tcp handler {handler_key} read agent data task error happen: {e:?}")
                }
            });
        }
        {
            let handler_key = handler_key.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::read_dst_data(&handler_key, dst_tcp_read, dst_to_agent_sender).await {
                    error!("Tcp handler {handler_key} read destination data task error happen: {e:?}")
                }
            });
        }
        {
            let handler_key = handler_key.clone();
            tokio::spawn(async move {
                if let Err(e) = Self::write_agent_data_to_dst(&handler_key, dst_tcp_write, agent_to_dst_receiver).await {
                    error!("Tcp handler {handler_key} write agent data to destination task error happen: {e:?}")
                }
            });
        }
        {
            tokio::spawn(async move {
                if let Err(e) =
                    Self::write_dst_data_to_agent(&handler_key, payload_encryption_token.clone(), agent_connection_write, dst_to_agent_receiver).await
                {
                    error!("Tcp handler {handler_key} write destination data to agent task error happen: {e:?}")
                }
            });
        }

        Ok(())
    }
}
