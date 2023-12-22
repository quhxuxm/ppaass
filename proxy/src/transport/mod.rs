mod tcp;
mod udp;

use anyhow::Result;
use futures::StreamExt;
use log::{debug, error, trace};
use pretty_hex::pretty_hex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::TcpStream;

use ppaass_common::tcp::AgentTcpPayload;
use ppaass_common::udp::AgentUdpPayload;
use ppaass_common::PpaassUnifiedAddress;
use ppaass_common::{agent::PpaassAgentConnection, random_32_bytes, PpaassAgentMessage, PpaassAgentMessagePayload, PpaassMessagePayloadEncryptionSelector};
use uuid::Uuid;

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
    transport_id: String,
    transport_number: Arc<AtomicU64>,
}

impl Transport {
    pub(crate) fn new(agent_tcp_stream: TcpStream, agent_address: PpaassUnifiedAddress, transport_number: Arc<AtomicU64>) -> Transport {
        let transport_id = Uuid::new_v4().to_string();
        let agent_connection = PpaassAgentConnection::new(
            transport_id.clone(),
            agent_tcp_stream,
            RSA_CRYPTO.clone(),
            PROXY_CONFIG.get_compress(),
            PROXY_CONFIG.get_agent_connection_codec_framed_buffer_size(),
        );
        debug!("Create transport [{transport_id}] for agent: {agent_address}");
        Self {
            agent_connection,
            transport_id,
            transport_number,
        }
    }

    pub(crate) async fn exec(mut self) -> Result<(), ProxyServerError> {
        //Read the first message from agent connection
        let transport_id = self.transport_id;
        let transport_number = self.transport_number;
        let agent_message = match self.agent_connection.next().await {
            Some(agent_message) => agent_message?,
            None => {
                error!("Transport {transport_id} closed in agent side, close proxy side also.",);
                return Ok(());
            },
        };
        let PpaassAgentMessage {
            user_token,
            message_id,
            payload,
            ..
        } = agent_message;
        let payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(random_32_bytes()));
        match payload {
            PpaassAgentMessagePayload::Tcp(payload_content) => {
                let AgentTcpPayload::Init { dst_address, src_address } = payload_content else {
                    error!("Transport {transport_id} expect to receive tcp init message but it is not: {payload_content:?}");
                    return Err(ProxyServerError::Other(format!(
                        "Transport {transport_id} expect to receive tcp init message but it is not"
                    )));
                };
                debug!("Transport {transport_id} receive tcp init message[{message_id}], src address: {src_address}, dst address: {dst_address}");
                // Tcp transport will block the thread and continue to
                // handle the agent connection in a loop
                if let Err(e) = TcpHandler::exec(
                    transport_id.clone(),
                    self.agent_connection,
                    user_token,
                    src_address,
                    dst_address,
                    payload_encryption,
                    transport_number.clone(),
                )
                .await
                {
                    transport_number.fetch_sub(1, Ordering::Relaxed);
                    error!(
                        "Transport {transport_id} error happen in tcp handler, current transport number: {}, error: {e:?}",
                        transport_number.load(Ordering::Relaxed)
                    );
                };
                Ok(())
            },
            PpaassAgentMessagePayload::Udp(payload_content) => {
                let AgentUdpPayload {
                    src_address,
                    dst_address,
                    data,
                    need_response,
                    ..
                } = payload_content;
                debug!("Transport {transport_id} receive udp data message[{message_id}], src address: {src_address}, dst address: {dst_address}");
                trace!("Transport {transport_id} receive udp data: {}", pretty_hex(&data));
                // Udp transport will block the thread and continue to
                // handle the agent connection in a loop
                if let Err(e) = UdpHandler::exec(
                    transport_id.clone(),
                    self.agent_connection,
                    user_token,
                    src_address,
                    dst_address,
                    data,
                    payload_encryption,
                    need_response,
                )
                .await
                {
                    transport_number.fetch_sub(1, Ordering::Relaxed);
                    error!(
                        "Transport {transport_id} error happen in udp handler, current transport number: {}, error: {e:?}",
                        transport_number.load(Ordering::Relaxed)
                    );
                    return Err(e);
                };
                transport_number.fetch_sub(1, Ordering::Relaxed);
                Ok(())
            },
        }
    }
}
