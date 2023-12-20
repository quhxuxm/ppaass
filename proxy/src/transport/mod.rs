mod tcp;
mod udp;

use anyhow::Result;
use futures::StreamExt;
use log::error;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

use ppaass_common::{
    agent::PpaassAgentConnection, random_32_bytes, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessageAgentTcpPayloadType,
    PpaassMessageAgentUdpPayloadType, PpaassMessagePayloadEncryptionSelector,
};
use ppaass_common::{tcp::AgentTcpInit, udp::AgentUdpData};
use ppaass_common::{PpaassMessageAgentProtocol, PpaassNetAddress};

use crate::{
    common::ProxyServerPayloadEncryptionSelector,
    config::PROXY_CONFIG,
    crypto::{ProxyServerRsaCryptoFetcher, RSA_CRYPTO},
    error::ProxyServerError,
    transport::udp::UdpHandler,
};

use self::tcp::TcpHandler;

pub(crate) struct Transport {
    agent_connection: PpaassAgentConnection<ProxyServerRsaCryptoFetcher>,
}

impl Transport {
    pub(crate) fn new(agent_tcp_stream: TcpStream, agent_address: PpaassNetAddress) -> Transport {
        let agent_connection = PpaassAgentConnection::new(
            agent_address.to_string(),
            agent_tcp_stream,
            RSA_CRYPTO.clone(),
            PROXY_CONFIG.get_compress(),
            PROXY_CONFIG.get_agent_receive_buffer_size(),
        );
        Self { agent_connection }
    }

    pub(crate) async fn exec(mut self) -> Result<(), ProxyServerError> {
        //Read the first message from agent connection
        let agent_message = match timeout(Duration::from_secs(PROXY_CONFIG.get_agent_relay_timeout()), self.agent_connection.next()).await {
            Err(_) => {
                error!("Read from agent timeout: {:?}", self.agent_connection.get_connection_id());
                return Err(ProxyServerError::Timeout(PROXY_CONFIG.get_agent_relay_timeout()));
            },
            Ok(Some(agent_message)) => agent_message?,
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
            id: agent_tcp_init_message_id,
            payload: PpaassAgentMessagePayload { protocol, data },
            ..
        } = agent_message;
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(random_32_bytes()));
        match protocol {
            PpaassMessageAgentProtocol::Tcp(payload_type) => {
                if PpaassMessageAgentTcpPayloadType::Init != payload_type {
                    return Err(ProxyServerError::Other(format!(
                        "Invalid tcp init payload type from agent message: {:?}",
                        payload_type
                    )));
                }
                let AgentTcpInit { src_address, dst_address } = data.try_into()?;
                // Tcp transport will block the thread and continue to
                // handle the agent connection in a loop
                TcpHandler::exec(
                    self.agent_connection,
                    agent_tcp_init_message_id,
                    user_token,
                    src_address,
                    dst_address,
                    payload_encryption,
                )
                .await?;
                Ok(())
            },
            PpaassMessageAgentProtocol::Udp(payload_type) => {
                if PpaassMessageAgentUdpPayloadType::Data != payload_type {
                    return Err(ProxyServerError::Other(format!(
                        "Invalid udp data payload type from agent message: {:?}",
                        payload_type
                    )));
                }
                let AgentUdpData {
                    src_address,
                    dst_address,
                    data: udp_raw_data,
                    need_response,
                    ..
                } = data.try_into()?;
                // Udp transport will block the thread and continue to
                // handle the agent connection in a loop
                UdpHandler::exec(
                    self.agent_connection,
                    user_token,
                    src_address,
                    dst_address,
                    udp_raw_data,
                    payload_encryption,
                    need_response,
                )
                .await?;
                Ok(())
            },
        }
    }
}
