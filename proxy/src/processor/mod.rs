use std::time::Duration;

use anyhow::{anyhow, Result};
use futures::StreamExt;
use log::{error, info};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::timeout;

use ppaass_common::{agent::PpaassAgentConnection, CommonError, PpaassAgentMessage, PpaassAgentMessagePayload};
use ppaass_common::{tcp::AgentTcpInit, udp::UdpData};
use ppaass_common::{PpaassMessageAgentPayloadType, PpaassNetAddress};

use crate::{
    config::PROXY_CONFIG,
    crypto::{ProxyServerRsaCryptoFetcher, RSA_CRYPTO},
    error::{NetworkError, ProxyError},
    processor::udp::{UdpHandler, UdpHandlerKey},
};

use self::tcp::{TcpHandler, TcpHandlerKey};

mod tcp;
mod udp;

pub(crate) struct AgentConnectionProcessor<'r, T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    'r: 'static,
{
    agent_connection: PpaassAgentConnection<'r, T, ProxyServerRsaCryptoFetcher, String>,
    agent_address: PpaassNetAddress,
}

impl<'r, T> AgentConnectionProcessor<'r, T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    'r: 'static,
{
    pub(crate) fn new(agent_tcp_stream: T, agent_address: PpaassNetAddress) -> AgentConnectionProcessor<'r, T> {
        let agent_connection = PpaassAgentConnection::new(
            agent_address.to_string(),
            agent_tcp_stream,
            &*RSA_CRYPTO,
            PROXY_CONFIG.get_compress(),
            PROXY_CONFIG.get_agent_recive_buffer_size(),
        );
        Self {
            agent_connection,
            agent_address,
        }
    }

    pub(crate) async fn exec(mut self) -> Result<(), ProxyError> {
        let agent_message = match timeout(Duration::from_secs(PROXY_CONFIG.get_agent_tcp_init_timeout()), self.agent_connection.next()).await {
            Err(_) => return Err(NetworkError::Timeout(PROXY_CONFIG.get_agent_tcp_init_timeout()).into()),
            Ok(Some(agent_message)) => agent_message?,
            Ok(None) => {
                error!(
                    "Agent connection {} closed in agent side, close the proxy side also.",
                    self.agent_connection.get_connection_id()
                );
                return Ok(());
            },
        };
        let PpaassAgentMessage {
            user_token,
            payload: PpaassAgentMessagePayload { payload_type, data },
            ..
        } = agent_message;

        match payload_type {
            PpaassMessageAgentPayloadType::TcpData => {
                Err(NetworkError::AgentRead(CommonError::Other(anyhow!("Receive unexpected payload type from agent"))).into())
            },
            PpaassMessageAgentPayloadType::TcpInit => {
                let tcp_init_request: AgentTcpInit = data.try_into()?;
                let src_address = tcp_init_request.src_address;
                let dst_address = tcp_init_request.dst_address;
                let tcp_handler_key = TcpHandlerKey::new(
                    self.agent_connection.get_connection_id().to_string(),
                    user_token,
                    self.agent_address,
                    src_address,
                    dst_address,
                );
                let tcp_handler = TcpHandler::new(tcp_handler_key, self.agent_connection);
                tcp_handler.exec().await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::UdpData => {
                info!("Agent connection {} receive udp data from agent.", self.agent_connection.get_connection_id());
                let UdpData {
                    src_address,
                    dst_address,
                    data: udp_raw_data,
                    ..
                } = data.try_into()?;
                let udp_handler_key = UdpHandlerKey::new(
                    self.agent_connection.get_connection_id().to_string(),
                    user_token,
                    self.agent_address,
                    src_address,
                    dst_address,
                );
                let udp_handler = UdpHandler::new(udp_handler_key, self.agent_connection);
                udp_handler.exec(udp_raw_data).await?;
                Ok(())
            },
        }
    }
}
