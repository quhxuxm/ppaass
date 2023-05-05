use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::anyhow;
use bytes::{Buf, BufMut, BytesMut};
use futures::StreamExt;
use futures_util::SinkExt;

use pretty_hex::pretty_hex;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::{
        oneshot::{error::TryRecvError, Receiver},
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
    agent_connection_buffer: Arc<Mutex<BytesMut>>,
    destination_connection_buffer: Arc<Mutex<BytesMut>>,
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
        let agent_connection_buffer = Arc::new(Mutex::new(BytesMut::with_capacity(1024 * 64)));
        let destination_connection_buffer = Arc::new(Mutex::new(BytesMut::with_capacity(1024 * 64)));
        Self {
            handler_key,
            agent_connection_read,
            agent_connection_write,
            user_token,
            configuration,
            src_address,
            dst_address,
            agent_connection_buffer,
            destination_connection_buffer,
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

    async fn write_dst_data_buffer_to_agent(
        handler_key: String, mut agent_tcp_write: PpaassConnectionWrite<T, R, I>, destination_connection_buffer: Arc<Mutex<BytesMut>>,
        mut destination_conntection_read_complete: Receiver<bool>,
    ) -> Result<(), ProxyError> {
        loop {
            let dst_read_complete = match destination_conntection_read_complete.try_recv() {
                Ok(_) => true,
                Err(TryRecvError::Empty) => false,
                Err(TryRecvError::Closed) => true,
            };
            let mut destination_connection_buffer = destination_connection_buffer.lock().await;
            if !destination_connection_buffer.has_remaining() && dst_read_complete {
               return Ok(()); 
            }
            agent_tcp_write.send(item).await?;
        }
    }
    async fn read_dst_data_to_buffer(
        handler_key: String, mut dst_tcp_read: DstConnectionRead<TcpStream, String>, destination_connection_buffer: Arc<Mutex<BytesMut>>,
    ) -> Result<(), ProxyError> {
        loop {
            let mut dst_message = match dst_tcp_read.next().await {
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
            while dst_message.has_remaining() {
                let mut destination_connection_buffer = destination_connection_buffer.lock().await;
                let remaining_space = destination_connection_buffer.capacity() - destination_connection_buffer.len();
                if dst_message.len() <= remaining_space {
                    let (data_read_to_buf, data_not_read_to_buf) = dst_message.split_at(remaining_space);
                    destination_connection_buffer.extend_from_slice(data_read_to_buf);
                    dst_message = data_not_read_to_buf.into();
                }
            }
        }
    }

    async fn read_agent_data_to_buffer(
        handler_key: String, mut agent_tcp_read: PpaassConnectionRead<T, R, I>, agent_connection_buffer: Arc<Mutex<BytesMut>>,
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
            let mut agent_message: BytesMut = BytesMut::from_iter(agent_message);
            while agent_message.has_remaining() {
                let mut agent_connection_buffer = agent_connection_buffer.lock().await;
                let remaining_space = agent_connection_buffer.capacity() - agent_connection_buffer.len();
                if agent_message.len() <= remaining_space {
                    let (data_read_to_buf, data_not_read_to_buf) = agent_message.split_at(remaining_space);
                    agent_connection_buffer.extend_from_slice(data_read_to_buf);
                    agent_message = data_not_read_to_buf.into();
                }
            }
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

        tokio::spawn(Self::read_agent_data_to_buffer(
            handler_key.clone(),
            agent_connection_read,
            self.agent_connection_buffer.clone(),
        ));
        tokio::spawn(Self::read_dst_data_to_buffer(
            handler_key.clone(),
            dst_tcp_read,
            self.destination_connection_buffer.clone(),
        ));
        loop {
            if stop_read_agent && stop_read_dst {
                break Ok(());
            }
            tokio::select! {
                agent_message = agent_connection_read.next(), if !stop_read_agent => {
                    let agent_message = match agent_message {
                        Some(Ok(agent_message)) => agent_message,
                        Some(Err(e))=>{
                            error!("Tcp handler {handler_key} fail to read agent message because of error: {e:?}");
                            if let Err(e) = dst_tcp_write.close().await {
                                error!("Tcp handler {handler_key} fail to close destination connection because of error: {e:?}");
                            };
                            stop_read_agent = true;
                            continue;
                        }
                        None => {
                            debug!("Tcp handler {handler_key} complete to read agent message.");
                            if let Err(e) = dst_tcp_write.close().await {
                                error!("Tcp handler {handler_key} fail to close destination connection because of error: {e:?}");
                            };
                            stop_read_agent = true;
                            continue;
                        },
                    };
                    let PpaassMessageParts { payload, .. } = agent_message.split();
                    let tcp_data: TcpData = match payload.try_into(){
                        Ok(tcp_data)=>tcp_data,
                        Err(e)=>{
                            error!("Tcp handler {handler_key} fail to relay agent message to destination because of can not parse tcp data error: {e:?}");
                            if let Err(e) = dst_tcp_write.close().await {
                                error!("Tcp handler {handler_key} fail to close destination connection because of error: {e:?}");
                            };
                            stop_read_agent = true;
                            continue;
                        }
                    };
                    let TcpDataParts{
                        raw_data,
                        src_address: src_address_in_data,
                        dst_address: dst_address_in_data
                    } = tcp_data.split();
                    if src_address != src_address_in_data{
                        error!("Tcp handler {handler_key} fail to relay agent message to destination because of src address is not the same.");
                        if let Err(e) = dst_tcp_write.close().await {
                            error!("Tcp handler {handler_key} fail to close destination connection because of error: {e:?}");
                        };
                        stop_read_agent = true;
                        continue;
                    }
                    if dst_address != dst_address_in_data{
                        error!("Tcp handler {handler_key} fail to relay agent message to destination because of dst address is not the same.");
                        if let Err(e) = dst_tcp_write.close().await {
                            error!("Tcp handler {handler_key} fail to close destination connection because of error: {e:?}");
                        };
                        stop_read_agent = true;
                        continue;
                    }
                    trace!("Tcp handler {handler_key} read agent data:\n{}\n", pretty_hex::pretty_hex(&raw_data));
                    let tcp_raw_data = BytesMut::from_iter(raw_data);
                    if let Err(e) = dst_tcp_write.send(tcp_raw_data).await {
                        error!("Tcp handler {handler_key} fail to relay agent message to destination because of error: {e:?}");
                        if let Err(e) = dst_tcp_write.close().await {
                            error!("Tcp handler {handler_key} fail to close destination connection because of error: {e:?}");
                        };
                        stop_read_agent = true;
                        continue;
                    };
                },

            }
        }
    }
}
