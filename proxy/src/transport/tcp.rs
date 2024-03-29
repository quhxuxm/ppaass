use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::{Bytes, BytesMut};

use futures::StreamExt as FuturesStreamExt;

use futures_util::SinkExt;

use log::{debug, error};
use tokio::net::TcpStream;

use ppaass_common::tcp::{AgentTcpPayload, ProxyTcpInitFailureReason, ProxyTcpInitResult};
use ppaass_common::{agent::PpaassAgentConnection, CommonError, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessagePayloadEncryption};
use ppaass_common::{PpaassMessageGenerator, PpaassUnifiedAddress};
use tokio::time::timeout;
use tokio_stream::StreamExt as TokioStreamExt;
use tokio_util::codec::{BytesCodec, Framed};

use crate::{config::PROXY_CONFIG, crypto::ProxyServerRsaCryptoFetcher, error::ProxyServerError};

pub(crate) struct TcpHandlerRequest {
    pub transport_id: String,
    pub agent_connection: PpaassAgentConnection<ProxyServerRsaCryptoFetcher>,
    pub user_token: String,
    pub src_address: PpaassUnifiedAddress,
    pub dst_address: PpaassUnifiedAddress,
    pub payload_encryption: PpaassMessagePayloadEncryption,
    pub transport_number: Arc<AtomicU64>,
}

#[derive(Default)]
pub(crate) struct TcpHandler;

impl TcpHandler {
    async fn init_dst_connection(transport_id: String, dst_address: &PpaassUnifiedAddress) -> Result<Framed<TcpStream, BytesCodec>, ProxyServerError> {
        let dst_socket_address = dst_address.to_socket_addrs()?.collect::<Vec<SocketAddr>>();
        let dst_tcp_stream = match timeout(
            Duration::from_secs(PROXY_CONFIG.get_dst_connect_timeout()),
            TcpStream::connect(dst_socket_address.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!(
                    "Transport {transport_id} connect to tcp destination [{dst_address}] timeout in [{}] seconds.",
                    PROXY_CONFIG.get_dst_connect_timeout()
                );
                return Err(ProxyServerError::Other(format!(
                    "Transport {transport_id} connect to tcp destination [{dst_address}] timeout in [{}] seconds.",
                    PROXY_CONFIG.get_dst_connect_timeout()
                )));
            },
            Ok(Ok(dst_tcp_stream)) => dst_tcp_stream,
            Ok(Err(e)) => {
                error!("Transport {transport_id} connect to tcp destination [{dst_address}] fail because of error: {e:?}");
                return Err(ProxyServerError::StdIo(e));
            },
        };
        dst_tcp_stream.set_nodelay(true)?;
        dst_tcp_stream.set_linger(None)?;
        // dst_tcp_stream.writable().await?;
        // dst_tcp_stream.readable().await?;
        let dst_connection = Framed::new(dst_tcp_stream, BytesCodec::new());
        Ok(dst_connection)
    }

    fn unwrap_to_raw_tcp_data(message: PpaassAgentMessage) -> Result<Bytes, CommonError> {
        let PpaassAgentMessage {
            payload: PpaassAgentMessagePayload::Tcp(AgentTcpPayload::Data { content }),
            ..
        } = message
        else {
            return Err(CommonError::Other(format!(
                "Fail to unwrap raw data from agent message because of invalid payload type: {message:?}"
            )));
        };
        Ok(content)
    }

    pub(crate) async fn exec(handler_request: TcpHandlerRequest) -> Result<(), ProxyServerError> {
        let TcpHandlerRequest {
            transport_id,
            mut agent_connection,
            user_token,
            src_address,
            dst_address,
            payload_encryption,
            transport_number,
        } = handler_request;
        let dst_connection = match Self::init_dst_connection(transport_id.clone(), &dst_address).await {
            Ok(dst_connection) => dst_connection,
            Err(e) => {
                error!(
                    "Transport {transport_id} can not connect to tcp destination [{dst_address}] because of error, current transport number:{}, error: {e:?}",
                    transport_number.load(Ordering::Relaxed)
                );
                let tcp_init_fail_message = PpaassMessageGenerator::generate_proxy_tcp_init_message(
                    user_token,
                    src_address,
                    dst_address,
                    payload_encryption,
                    ProxyTcpInitResult::Fail(ProxyTcpInitFailureReason::CanNotConnectToDestination),
                )?;
                agent_connection.send(tcp_init_fail_message).await?;
                return Err(e);
            },
        };
        debug!("Transport {transport_id} success connect to tcp destination: {dst_address}");
        let tcp_init_success_message = PpaassMessageGenerator::generate_proxy_tcp_init_message(
            user_token.clone(),
            src_address.clone(),
            dst_address.clone(),
            payload_encryption.clone(),
            ProxyTcpInitResult::Success(transport_id.clone()),
        )?;
        agent_connection.send(tcp_init_success_message).await?;
        debug!("Transport {transport_id} sent tcp init success message to agent.");
        let (mut agent_connection_write, agent_connection_read) = agent_connection.split();
        let (mut dst_connection_write, dst_connection_read) = dst_connection.split();
        debug!("Transport {transport_id} start task to relay agent and tcp destination: {dst_address}");
        {
            let dst_address = dst_address.clone();
            let transport_id = transport_id.clone();
            let transport_number = transport_number.clone();
            tokio::spawn(async move {
                if let Err(e) = TokioStreamExt::map_while(agent_connection_read, |agent_message| {
                    let agent_message = agent_message.ok()?;
                    let data = Self::unwrap_to_raw_tcp_data(agent_message).ok()?;
                    Some(Ok(BytesMut::from_iter(data)))
                })
                .forward(&mut dst_connection_write)
                .await
                {
                    error!("Transport {transport_id} error happen when relay tcp data from agent to destination [{dst_address}], current transport number: {}, error: {e:?}",transport_number.load(Ordering::Relaxed));
                }
                if let Err(e) = dst_connection_write.close().await {
                    error!("Transport {transport_id} fail to close destination connection [{dst_address}] beccause of error, current transport number: {}, error: {e:?}",transport_number.load(Ordering::Relaxed));
                };
            });
        }
        tokio::spawn(async move {
            if let Err(e) = TokioStreamExt::map_while(dst_connection_read, move |dst_message| {
                let dst_message = dst_message.ok()?;
                let tcp_data_message =
                    PpaassMessageGenerator::generate_proxy_tcp_data_message(user_token.clone(), payload_encryption.clone(), dst_message.freeze()).ok()?;
                Some(Ok(tcp_data_message))
            })
            .forward(&mut agent_connection_write)
            .await
            {
                error!("Transport {transport_id} error happen when relay tcp data from destination [{dst_address}] to agent, current transport number: {}, error: {e:?}", transport_number.load(Ordering::Relaxed));
            }
            transport_number.fetch_sub(1, Ordering::Relaxed);
            if let Err(e) = agent_connection_write.close().await {
                error!(
                    "Transport {transport_id} fail to close agent connection beccause of error, current transport number: {}, error: {e:?}",
                    transport_number.load(Ordering::Relaxed)
                );
            };
        });
        Ok(())
    }
}
