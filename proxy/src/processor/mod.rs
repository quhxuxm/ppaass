use std::time::Duration;

use anyhow::Result;
use futures::StreamExt;
use log::error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::timeout;

use ppaass_common::{
    agent::PpaassAgentConnection, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessageAgentTcpPayloadType, PpaassMessageAgentUdpPayloadType,
};
use ppaass_common::{tcp::AgentTcpInit, udp::UdpData};
use ppaass_common::{PpaassMessageAgentProtocol, PpaassNetAddress};

use crate::{
    config::PROXY_CONFIG,
    crypto::{ProxyServerRsaCryptoFetcher, RSA_CRYPTO},
    error::ProxyError,
    processor::udp::UdpHandler,
};

use self::tcp::TcpHandler;

mod tcp;
mod udp;

pub(crate) struct Transport<'r, T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    'r: 'static,
{
    agent_connection: PpaassAgentConnection<'r, T, ProxyServerRsaCryptoFetcher, String>,
}

impl<'r, T> Transport<'r, T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    'r: 'static,
{
    pub(crate) fn new(agent_tcp_stream: T, agent_address: PpaassNetAddress) -> Transport<'r, T> {
        let agent_connection = PpaassAgentConnection::new(
            agent_address.to_string(),
            agent_tcp_stream,
            &*RSA_CRYPTO,
            PROXY_CONFIG.get_compress(),
            PROXY_CONFIG.get_agent_recive_buffer_size(),
        );
        Self { agent_connection }
    }

    pub(crate) async fn exec(mut self) -> Result<(), ProxyError> {
        let agent_message = match timeout(Duration::from_secs(PROXY_CONFIG.get_agent_relay_timeout()), self.agent_connection.next()).await {
            Err(_) => {
                error!("Read from agent timeout: {:?}", self.agent_connection.get_connection_id());
                return Err(ProxyError::Timeout(PROXY_CONFIG.get_agent_relay_timeout()));
            },
            Ok(Some(agent_message)) => agent_message.map_err(ProxyError::Common)?,
            Ok(None) => {
                error!(
                    "Transport {} closed in agent side, close proxy side also.",
                    self.agent_connection.get_connection_id()
                );
                return Ok(());
            },
        };
        let PpaassAgentMessage {
            user_token,
            payload: PpaassAgentMessagePayload { protocol, data },
            ..
        } = agent_message;

        match protocol {
            PpaassMessageAgentProtocol::Tcp(payload_type) => {
                if PpaassMessageAgentTcpPayloadType::Init != payload_type {
                    return Err(ProxyError::Other(format!(
                        "Invalid tcp init payload type from agent message: {:?}",
                        payload_type
                    )));
                }
                let AgentTcpInit { src_address, dst_address } = data.try_into().map_err(ProxyError::Common)?;
                // Tcp handler will block the thread and continue to
                // handle the agent connection in a loop
                TcpHandler::exec(self.agent_connection, user_token, src_address, dst_address).await?;
                Ok(())
            },
            PpaassMessageAgentProtocol::Udp(payload_type) => {
                if PpaassMessageAgentUdpPayloadType::Data != payload_type {
                    return Err(ProxyError::Other(format!(
                        "Invalid udp data payload type from agent message: {:?}",
                        payload_type
                    )));
                }
                let UdpData {
                    src_address,
                    dst_address,
                    data: udp_raw_data,
                    ..
                } = data.try_into().map_err(ProxyError::Common)?;
                // Udp handler will block the thread and continue to
                // handle the agent connection in a loop
                UdpHandler::exec(self.agent_connection, user_token, src_address, dst_address, udp_raw_data).await?;
                Ok(())
            },
        }
    }
}
