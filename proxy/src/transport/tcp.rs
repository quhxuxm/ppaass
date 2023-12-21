use std::{
    net::{SocketAddr, ToSocketAddrs},
    time::Duration,
};

use bytes::{Bytes, BytesMut};

use futures::StreamExt as FuturesStreamExt;

use futures_util::SinkExt;

use log::{debug, error};
use tokio::net::TcpStream;

use ppaass_common::tcp::AgentTcpPayload;
use ppaass_common::{
    agent::PpaassAgentConnection, tcp::ProxyTcpInitResultType, CommonError, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessagePayloadEncryption,
};
use ppaass_common::{PpaassMessageGenerator, PpaassUnifiedAddress};
use tokio::time::timeout;
use tokio_stream::StreamExt as TokioStreamExt;
use tokio_util::codec::{BytesCodec, Framed};

use crate::{config::PROXY_CONFIG, crypto::ProxyServerRsaCryptoFetcher, error::ProxyServerError};

#[derive(Default)]
#[non_exhaustive]
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
                error!("Transport {transport_id} connect to tcp destination timeout: {dst_address}");
                return Err(ProxyServerError::Timeout(PROXY_CONFIG.get_dst_connect_timeout()));
            },
            Ok(Ok(dst_tcp_stream)) => dst_tcp_stream,
            Ok(Err(e)) => {
                error!("Transport {transport_id} connect to tcp destination [{dst_address}] fail because of error: {e:?}");
                return Err(ProxyServerError::GeneralIo(e));
            },
        };
        dst_tcp_stream.set_nodelay(true)?;
        dst_tcp_stream.set_linger(None)?;
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

    pub(crate) async fn exec(
        transport_id: String, mut agent_connection: PpaassAgentConnection<ProxyServerRsaCryptoFetcher>, agent_tcp_init_message_id: String, user_token: String,
        src_address: PpaassUnifiedAddress, dst_address: PpaassUnifiedAddress, payload_encryption: PpaassMessagePayloadEncryption,
    ) -> Result<(), ProxyServerError> {
        let dst_connection = match Self::init_dst_connection(transport_id.clone(), &dst_address).await {
            Ok(dst_connection) => dst_connection,
            Err(e) => {
                error!("Transport {transport_id} can not connect to tcp destination because of error: {e:?}");
                let tcp_init_fail_message = PpaassMessageGenerator::generate_proxy_tcp_init_message(
                    agent_tcp_init_message_id,
                    user_token,
                    src_address,
                    dst_address,
                    payload_encryption,
                    ProxyTcpInitResultType::ConnectToDstFail,
                )?;
                agent_connection.send(tcp_init_fail_message).await?;
                return Err(e);
            },
        };
        debug!("Transport {transport_id} success connect to tcp destination: {dst_address}");
        let tcp_init_success_message = PpaassMessageGenerator::generate_proxy_tcp_init_message(
            agent_tcp_init_message_id,
            user_token.clone(),
            src_address.clone(),
            dst_address.clone(),
            payload_encryption.clone(),
            ProxyTcpInitResultType::Success,
        )?;
        agent_connection.send(tcp_init_success_message).await?;
        debug!("Transport {transport_id} sent tcp init success message to agent.");
        let (agent_connection_write, agent_connection_read) = agent_connection.split();
        let (dst_connection_write, dst_connection_read) = dst_connection.split();
        debug!("Transport {transport_id} start task forward agent data to tcp destination: {dst_address}");
        tokio::spawn(
            TokioStreamExt::map_while(agent_connection_read, |agent_message| {
                let agent_message = agent_message.ok()?;
                let data = Self::unwrap_to_raw_tcp_data(agent_message).ok()?;
                Some(Ok(BytesMut::from_iter(data)))
            })
            .forward(dst_connection_write),
        );
        debug!("Transport {transport_id} start task forward tcp destination data to agent: {dst_address}");
        tokio::spawn(
            TokioStreamExt::map_while(dst_connection_read, move |dst_message| {
                let dst_message = dst_message.ok()?;
                let tcp_data_message =
                    PpaassMessageGenerator::generate_proxy_tcp_data_message(user_token.clone(), payload_encryption.clone(), dst_message.freeze()).ok()?;
                Some(Ok(tcp_data_message))
            })
            .forward(agent_connection_write),
        );
        Ok(())
    }
}
