use std::fmt::Debug;
use std::fmt::Display;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context as AnyhowContext, Result};
use bytes::BytesMut;
use futures::StreamExt;
use futures_util::SinkExt;

use tokio::time::timeout;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use tracing::{debug, error, trace};

use ppaass_common::{
    generate_uuid,
    tcp::{TcpData, TcpDataParts, TcpInitResponseType},
    PpaassConnectionRead, PpaassConnectionWrite, PpaassMessageParts, RsaCryptoFetcher,
};
use ppaass_common::{PpaassMessageGenerator, PpaassMessagePayloadEncryptionSelector, PpaassNetAddress};

use crate::{common::ProxyServerPayloadEncryptionSelector, config::ProxyServerConfig};

use super::destination::{DstConnection, DstConnectionParts};

pub(crate) struct TcpHandlerBuilder<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    ppaass_connection_read: Option<PpaassConnectionRead<T, R, I>>,
    ppaass_connection_write: Option<PpaassConnectionWrite<T, R, I>>,
    user_token: Option<String>,
    agent_address: Option<PpaassNetAddress>,
    src_address: Option<PpaassNetAddress>,
    dst_address: Option<PpaassNetAddress>,
    ppaass_connection_id: Option<String>,
}

impl<T, R, I> TcpHandlerBuilder<T, R, I>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    pub(crate) fn new() -> Self {
        Self {
            agent_address: None,
            src_address: None,
            dst_address: None,
            ppaass_connection_read: None,
            ppaass_connection_write: None,
            user_token: None,
            ppaass_connection_id: None,
        }
    }

    pub(crate) fn user_token(mut self, user_token: impl AsRef<str>) -> TcpHandlerBuilder<T, R, I> {
        self.user_token = Some(user_token.as_ref().to_owned());
        self
    }

    pub(crate) fn agent_address(mut self, agent_address: PpaassNetAddress) -> TcpHandlerBuilder<T, R, I> {
        self.agent_address = Some(agent_address);
        self
    }

    pub(crate) fn src_address(mut self, src_address: PpaassNetAddress) -> TcpHandlerBuilder<T, R, I> {
        self.src_address = Some(src_address);
        self
    }

    pub(crate) fn dst_address(mut self, dst_address: PpaassNetAddress) -> TcpHandlerBuilder<T, R, I> {
        self.dst_address = Some(dst_address);
        self
    }

    pub(crate) fn ppaass_connection_read(mut self, ppaass_connection_read: PpaassConnectionRead<T, R, I>) -> TcpHandlerBuilder<T, R, I> {
        self.ppaass_connection_read = Some(ppaass_connection_read);
        self
    }

    pub(crate) fn ppaass_connection_write(mut self, ppaass_connection_write: PpaassConnectionWrite<T, R, I>) -> TcpHandlerBuilder<T, R, I> {
        self.ppaass_connection_write = Some(ppaass_connection_write);
        self
    }

    pub(crate) fn ppaass_connection_id(mut self, ppaass_connection_id: String) -> TcpHandlerBuilder<T, R, I> {
        self.ppaass_connection_id = Some(ppaass_connection_id);
        self
    }

    fn generate_handler_key(
        user_token: &str, agent_address: &PpaassNetAddress, src_address: &PpaassNetAddress, dst_address: &PpaassNetAddress, ppaass_connection_id: &str,
    ) -> String {
        format!("[{ppaass_connection_id}]#[{user_token}]@TCP::[{agent_address}]::[{src_address}=>{dst_address}]")
    }

    pub(crate) async fn build(self, configuration: Arc<ProxyServerConfig>) -> Result<TcpHandler<T, R, I>> {
        let agent_address = self.agent_address.context("Agent address not assigned for tcp handler builder")?;
        let src_address = self.src_address.context("Source address not assigned for tcp handler builder")?;
        let dst_address = self.dst_address.context("Destination address not assigned for tcp handler builder")?;
        let user_token = self.user_token.context("User token not assigned for tcp handler builder")?;
        let ppaass_connection_id = self.ppaass_connection_id.context("Ppaass connection id not assigned for tcp handler builder")?;
        let handler_key = Self::generate_handler_key(&user_token, &agent_address, &src_address, &dst_address, &ppaass_connection_id);
        let ppaass_connection_write = self
            .ppaass_connection_write
            .context("Agent message framed write not assigned for tcp handler builder")?;
        let ppaass_connection_read = self
            .ppaass_connection_read
            .context("Agent message framed read not assigned for tcp handler builder")?;
        Ok(TcpHandler {
            handler_key,
            ppaass_connection_read,
            ppaass_connection_write,
            user_token,
            configuration,
            src_address,
            dst_address,
        })
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
    src_address: PpaassNetAddress,
    dst_address: PpaassNetAddress,
    ppaass_connection_read: PpaassConnectionRead<T, R, I>,
    ppaass_connection_write: PpaassConnectionWrite<T, R, I>,
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
    pub(crate) async fn exec(self) -> Result<()> {
        let handler_key = self.handler_key.clone();
        let src_address = self.src_address;
        let dst_address = self.dst_address;
        let user_token = self.user_token;
        let mut ppaass_connection_write = self.ppaass_connection_write;
        let mut ppaass_connection_read = self.ppaass_connection_read;

        let dst_socket_address = dst_address
            .to_socket_addrs()
            .map_err(|e| {
                error!("Fail to convert dest address [{dst_address}] to valid socket address because of error:{e:?}");
                anyhow!("Fail to convert dest address [{dst_address}] to valid socket address because of error:{e:?}")
            })?
            .collect::<Vec<SocketAddr>>();

        let dst_tcp_stream = match timeout(
            Duration::from_secs(self.configuration.get_dst_connect_timeout()),
            TcpStream::connect(dst_socket_address.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Tcp handler {handler_key} fail connect to destination [{dst_address}] because of timeout.");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_init_fail_message = PpaassMessageGenerator::generate_tcp_init_response(
                    handler_key.clone(),
                    &user_token,
                    src_address,
                    dst_address,
                    payload_encryption_token,
                    TcpInitResponseType::Fail,
                )?;
                ppaass_connection_write
                    .send(tcp_init_fail_message)
                    .await
                    .context(format!("Tcp handler {handler_key} fail to send tcp loop init fail response to agent."))?;
                return Err(anyhow!("Tcp handler {handler_key} fail connect to dest address because of timeout."));
            },
            Ok(Ok(dst_tcp_stream)) => dst_tcp_stream,
            Ok(Err(e)) => {
                error!("Tcp handler {handler_key} fail connect to dest address [{dst_address}] because of error: {e:?}");
                let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let tcp_initialize_fail_message = PpaassMessageGenerator::generate_tcp_init_response(
                    handler_key.clone(),
                    &user_token,
                    src_address,
                    dst_address,
                    payload_encryption_token,
                    TcpInitResponseType::Fail,
                )?;
                ppaass_connection_write
                    .send(tcp_initialize_fail_message)
                    .await
                    .context(format!("Tcp handler {handler_key} fail to send tcp loop init fail response to agent."))?;
                return Err(anyhow!(e));
            },
        };
        dst_tcp_stream.set_nodelay(true)?;
        dst_tcp_stream.set_linger(None)?;
        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let tcp_init_success_message = PpaassMessageGenerator::generate_tcp_init_response(
            handler_key.clone(),
            &user_token,
            src_address.clone(),
            dst_address.clone(),
            payload_encryption_token,
            TcpInitResponseType::Success,
        )?;
        if let Err(e) = ppaass_connection_write.send(tcp_init_success_message).await {
            error!("Tcp handler {handler_key} fail to send tcp initialize success message to agent because of error: {e:?}");
            return Err(anyhow!(e));
        };

        let payload_encryption_token = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
        let mut stop_read_agent = false;
        let mut stop_read_dst = false;

        let dst_connection = DstConnection::new(handler_key.clone(), dst_tcp_stream, self.configuration.get_dst_tcp_buffer_size());
        let DstConnectionParts {
            read: mut dst_tcp_read,
            write: mut dst_tcp_write,
            id: dst_connection_id,
        } = dst_connection.split();
        debug!("Tcp handler {handler_key} create destination connection: {dst_connection_id}");
        loop {
            if stop_read_agent && stop_read_dst {
                if let Err(e) = ppaass_connection_write.close().await {
                    error!("Tcp handler {handler_key} fail to close agent connection because of error: {e:?}");
                };
                if let Err(e) = dst_tcp_write.close().await {
                    error!("Tcp handler {handler_key} fail to close destination connection because of error: {e:?}");
                };
                break Ok(());
            }
            tokio::select! {
                agent_message = ppaass_connection_read.next(), if !stop_read_agent => {
                    let agent_message = match agent_message {
                        Some(Ok(agent_message)) => agent_message,
                        Some(Err(e))=>{
                            error!("Tcp handler {handler_key} fail to read agent message because of error: {e:?}");
                            stop_read_agent = true;
                            continue;
                        }
                        None => {
                            debug!("Tcp handler {handler_key} complete to read agent message.");
                            stop_read_agent = true;
                            continue;
                        },
                    };
                    let PpaassMessageParts { payload, .. } = agent_message.split();
                    let tcp_data: TcpData = match payload.try_into(){
                        Ok(tcp_data)=>tcp_data,
                        Err(e)=>{
                            error!("Tcp handler {handler_key} fail to relay agent message to destination because of can not parse tcp data error: {e:?}");
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
                        stop_read_agent = true;
                        continue;
                    }
                    if dst_address != dst_address_in_data{
                        error!("Tcp handler {handler_key} fail to relay agent message to destination because of dst address is not the same.");
                        stop_read_agent = true;
                        continue;
                    }
                    trace!("Tcp handler {handler_key} read agent data:\n{}\n", pretty_hex::pretty_hex(&raw_data));
                    let tcp_raw_data = BytesMut::from_iter(raw_data);
                    if let Err(e) = dst_tcp_write.send(tcp_raw_data).await {
                        error!("Tcp handler {handler_key} fail to relay agent message to destination because of error: {e:?}");
                        stop_read_agent = true;
                        continue;
                    };
                },
                dst_message = dst_tcp_read.next(), if !stop_read_dst => {
                    let dst_message = match dst_message {
                        None => {
                            debug!("Tcp handler {handler_key} complete to read destination data.");
                            stop_read_dst=true;
                            continue;
                        },
                        Some(Ok(dst_message)) => dst_message,
                        Some(Err(e)) => {
                            error!("Tcp handler {handler_key} fail to read destination data because of error: {e:?}");
                            stop_read_dst=true;
                            continue;
                        },
                    };
                    trace!("Tcp handler {handler_key} read destination data:\n{}\n", pretty_hex::pretty_hex(&dst_message));
                    let tcp_data_message = match PpaassMessageGenerator::generate_tcp_data(&user_token, payload_encryption_token.clone(),src_address.clone(), dst_address.clone(),  dst_message.to_vec()) {
                        Ok(tcp_data_message) => tcp_data_message,
                        Err(e) => {
                            error!("Tcp handler {handler_key} fail to generate raw data because of error: {e:?}");
                            stop_read_dst=true;
                            continue;
                        },
                    };
                    if let Err(e) = ppaass_connection_write.send(tcp_data_message).await {
                        error!("Tcp handler {handler_key} fail to relay destination data to agent because of error: {e:?}");
                        stop_read_dst=true;
                        continue;
                    };
                }
            }
        }
    }
}
