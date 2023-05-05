use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::StreamExt;
use futures_util::SinkExt;

use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::{
        mpsc::{Receiver, Sender},
        Mutex,
    },
};
use tokio::{sync::mpsc::channel, time::timeout};
use tracing::{debug, error, trace};

use ppaass_common::{
    generate_uuid,
    tcp::{TcpData, TcpDataParts, TcpInitResponseType},
    PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageParts, PpaassMessagePayloadEncryption, RsaCryptoFetcher,
};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig, error::ProxyError};

use super::destination::{DstConnection, DstConnectionParts, DstConnectionRead, DstConnectionWrite};

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct TcpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    agent_connection_read: PpaassConnectionRead<T, R, I>,
    agent_connection_write: PpaassConnectionWrite<T, R, I>,
    handler_key: String,
    user_token: String,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R, I> TcpHandler<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    fn generate_handler_key(
        user_token: &str, agent_address: &PpaassNetAddress, src_address: &PpaassNetAddress, dst_address: &PpaassNetAddress, connection_id: &str,
    ) -> String {
        format!("[{connection_id}]#[{user_token}]@TCP::[{agent_address}]::[{src_address}=>{dst_address}]")
    }

    pub(crate) fn new(
        connection_id: String, user_token: String, agent_connection_read: PpaassConnectionRead<T, R, I>,
        agent_connection_write: PpaassConnectionWrite<T, R, I>, agent_address: PpaassNetAddress, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        configuration: Arc<ProxyServerConfig>,
    ) -> Self {
        let handler_key = Self::generate_handler_key(&user_token, &agent_address, &src_address, &dst_address, &connection_id);
        Self {
            handler_key,
            agent_connection_read,
            agent_connection_write,
            user_token,
            configuration,
            src_address,
            dst_address,
        }
    }

    async fn init_dst_connection(
        handler_key: String, dst_address: &PpaassNetAddress, configuration: Arc<ProxyServerConfig>,
    ) -> Result<(DstConnectionRead<TcpStream, String>, DstConnectionWrite<TcpStream, String>), ProxyError> {
        let dst_socket_address = dst_address.to_socket_addrs()?.collect::<Vec<SocketAddr>>();

        let dst_tcp_stream = match timeout(
            Duration::from_secs(configuration.get_dst_connect_timeout()),
            TcpStream::connect(dst_socket_address.as_slice()),
        )
        .await
        {
            Err(timeout) => {
                error!("Tcp handler {handler_key} fail connect to destination [{dst_address}] because of timeout.");
                return Err(ProxyError::Io(timeout.into()));
            },
            Ok(Ok(dst_tcp_stream)) => dst_tcp_stream,
            Ok(Err(e)) => {
                error!("Tcp handler {handler_key} fail connect to dest address [{dst_address}] because of error: {e:?}");
                return Err(e.into());
            },
        };
        dst_tcp_stream.set_nodelay(true)?;
        dst_tcp_stream.set_linger(None)?;
        let dst_connection = DstConnection::new(handler_key.clone(), dst_tcp_stream, configuration.get_dst_tcp_buffer_size());
        let DstConnectionParts {
            read: dst_tcp_read,
            write: dst_tcp_write,
            id: dst_connection_id,
        } = dst_connection.split();
        Ok((dst_tcp_read, dst_tcp_write))
    }

    async fn write_dst_data_to_agent(
        handler_key: String, user_token: String, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, mut agent_tcp_write: PpaassConnectionWrite<T, R, I>, mut dst_to_agent_receiver: Receiver<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let dst_data = match dst_to_agent_receiver.recv().await {
                Some(dst_data) => dst_data,
                None => {
                    return Ok(());
                },
            };
            let tcp_data_message = PpaassMessageGenerator::generate_tcp_data(
                user_token.clone(),
                payload_encryption.clone(),
                src_address.clone(),
                dst_address.clone(),
                dst_data.to_vec(),
            )?;
            agent_tcp_write.send(tcp_data_message).await?;
        }
    }

    async fn write_agent_data_to_dst(
        handler_key: String, user_token: String, src_address: PpaassNetAddress, dst_address: PpaassNetAddress,
        payload_encryption: PpaassMessagePayloadEncryption, mut dst_tcp_write: DstConnectionWrite<TcpStream, String>,
        mut agent_to_dst_receiver: Receiver<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let agent_data = match agent_to_dst_receiver.recv().await {
                Some(agent_data) => agent_data,
                None => {
                    return Ok(());
                },
            };
            dst_tcp_write.send(agent_data).await?;
        }
    }

    async fn read_dst_data(
        handler_key: String, mut dst_tcp_read: DstConnectionRead<TcpStream, String>, dst_to_agent_sender: Sender<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let dst_message = match dst_tcp_read.next().await {
                Some(Ok(dst_message)) => dst_message,
                Some(Err(e)) => {
                    error!("Tcp handler {handler_key} fail to read destination data because of error: {e:?}");
                    return Err(e);
                },
                None => {
                    debug!("Tcp handler {handler_key} complete to read destination data.");
                    return Ok(());
                },
            };
            dst_to_agent_sender.send(dst_message.into()).await.map_err(|e| ProxyError::Other(anyhow!(e)))?;
        }
    }

    async fn read_agent_data(
        handler_key: String, mut agent_tcp_read: PpaassConnectionRead<T, R, I>, agent_to_dst_sender: Sender<BytesMut>,
    ) -> Result<(), ProxyError> {
        loop {
            let agent_message = match agent_tcp_read.next().await {
                Some(Ok(agent_message)) => agent_message,
                Some(Err(e)) => {
                    error!("Tcp handler {handler_key} fail to read agent data because of error: {e:?}");
                    return Err(e.into());
                },
                None => {
                    debug!("Tcp handler {handler_key} complete to read agent data.");
                    return Ok(());
                },
            };
            let PpaassMessageParts { payload: agent_message, .. } = agent_message.split();
            let TcpData {
                src_address,
                dst_address,
                raw_data,
                ..
            } = agent_message.try_into()?;
            let raw_data = BytesMut::from_iter(raw_data);
            agent_to_dst_sender.send(raw_data).await.map_err(|e| ProxyError::Other(anyhow!(e)))?;
        }
    }

    pub(crate) async fn exec(self) -> Result<(), ProxyError> {
        let handler_key = self.handler_key.clone();
        let src_address = self.src_address;
        let dst_address = self.dst_address;
        let user_token = self.user_token;
        let mut agent_connection_write = self.agent_connection_write;
        let mut agent_connection_read = self.agent_connection_read;

        let (dst_tcp_read, dst_tcp_write) = match Self::init_dst_connection(handler_key, &dst_address, self.configuration).await {
            Ok(dst_read_and_write) => dst_read_and_write,
            Err(e) => {
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_init_fail = PpaassMessageGenerator::generate_tcp_init_response(
                    handler_key.clone(),
                    &user_token,
                    src_address.clone(),
                    dst_address.clone(),
                    payload_encryption_token,
                    TcpInitResponseType::Fail,
                )?;
                agent_connection_write.send(tcp_init_fail).await?;
                return Err(e);
            },
        };
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_init_success_message = PpaassMessageGenerator::generate_tcp_init_response(
            handler_key.clone(),
            &user_token,
            src_address.clone(),
            dst_address.clone(),
            payload_encryption_token,
            TcpInitResponseType::Success,
        )?;
        agent_connection_write.send(tcp_init_success_message).await?;
        debug!("Tcp handler {handler_key} create destination connection success.");
        let (agent_to_dst_sender, agent_to_dst_receiver) = channel(1024);
        let (dst_to_agent_sender, dst_to_agent_receiver) = channel(1024);
        tokio::spawn(Self::read_agent_data(handler_key.clone(), agent_connection_read, agent_to_dst_sender));
        tokio::spawn(Self::read_dst_data(handler_key.clone(), dst_tcp_read, dst_to_agent_sender));
        tokio::spawn(Self::write_agent_data_to_dst(handler_key.clone(), agent_connection_read, agent_to_dst_sender));
        tokio::spawn(Self::read_dst_data(handler_key.clone(), dst_tcp_read, dst_to_agent_sender));
        Ok(())
    }
}
