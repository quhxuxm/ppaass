mod tcp;
mod udp;

use anyhow::Result;
use futures::StreamExt;
use log::error;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

use ppaass_common::tcp::AgentTcpPayload;
use ppaass_common::udp::AgentUdpData;
use ppaass_common::PpaassUnifiedAddress;
use ppaass_common::{agent::PpaassAgentConnection, random_32_bytes, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessagePayloadEncryptionSelector};

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
    pub(crate) fn new(agent_tcp_stream: TcpStream, agent_address: PpaassUnifiedAddress) -> Transport {
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
            message_id: agent_tcp_init_message_id,
            payload: agent_message_payload,
            ..
        } = agent_message;
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(random_32_bytes()));
        match agent_message_payload {
            PpaassAgentMessagePayload::Tcp(payload_content) => {
                let AgentTcpPayload::Init { dst_address, src_address } = payload_content else {
                    return Err(ProxyServerError::Other(format!(
                        "Expect tcp init from agent but get other message: {payload_content:?}"
                    )));
                };
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
            PpaassAgentMessagePayload::Udp(payload_content) => {
                let AgentUdpData {
                    src_address,
                    dst_address,
                    data: udp_raw_data,
                    need_response,
                    ..
                } = payload_content;
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
