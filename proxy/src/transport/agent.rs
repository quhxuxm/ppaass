use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use anyhow::{anyhow, Result};

use futures::{SinkExt, TryStreamExt};
use ppaass_common::{generate_uuid, PpaassError};

use ppaass_protocol::{
    PpaassMessage, PpaassMessageAgentPayloadTypeValue, PpaassMessageParts, PpaassMessagePayload, PpaassMessagePayloadParts, PpaassMessagePayloadType,
};
use tokio::net::TcpStream;
use tracing::{error, info};

use crate::{common::AgentTcpConnection, config::ProxyServerConfig};

use super::TargetTransportOutboundValue;

#[derive(Debug)]
pub(crate) struct AgentTransport {
    id: String,
    agent_tcp_connection: AgentTcpConnection,
}

impl AgentTransport {
    pub(crate) fn new(agent_tcp_connection: AgentTcpConnection, _configuration: Arc<ProxyServerConfig>) -> Self {
        let (outbound_sender, outbound_receiver) = channel::<TargetTransportOutboundValue>(1024);
        Self {
            id: generate_uuid(),
            agent_tcp_connection,
        }
    }

    pub(crate) fn get_id(&self) -> &str {
        &self.id
    }

    pub(crate) async fn exec(self) -> Result<()> {
        let mut agent_tcp_connection = self.agent_tcp_connection;
        tokio::spawn(async move {
            loop {
                let agent_message = agent_tcp_connection.try_next().await?;
                let PpaassMessageParts { payload_bytes, user_token, .. } = match agent_message {
                    None => {
                        info!("Agent tcp connection [{agent_tcp_connection:?}] disconnected.");
                        return Ok(());
                    },
                    Some(v) => v.split(),
                };
                let agent_message_payload: PpaassMessagePayload = payload_bytes.try_into()?;
                let PpaassMessagePayloadParts {
                    connection_id,
                    payload_type,
                    source_address,
                    target_address,
                    additional_info,
                    data,
                } = agent_message_payload.split();
                match payload_type {
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::DomainNameResolve) => {},
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::ConnectionKeepAlive) => {
                        let keep_alive_success_message_payload = PpaassMessagePayload::new(None, source_address, target_address, payload_type, data);
                        let payload_bytes: Vec<u8> = keep_alive_success_message_payload.try_into()?;
                        let keep_alive_success_message = PpaassMessage::new(
                            user_token,
                            ppaass_protocol::PpaassMessagePayloadEncryption::Aes(generate_uuid().as_bytes().to_vec()),
                            payload_bytes,
                        );
                        if let Err(e) = agent_tcp_connection.send(keep_alive_success_message).await {
                            error!("Fail to do keep alive for agent tcp connection [{agent_tcp_connection:?}], error: {e:?}");
                            return Err(anyhow!(e));
                        };
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize) => {
                        let target_address = target_address.ok_or(PpaassError::CodecError)?;
                        let target_socket_addrs = target_address.to_socket_addrs()?;
                        let target_socket_addrs = target_socket_addrs.collect::<Vec<SocketAddr>>();
                        let target_tcp_stream = match TcpStream::connect(target_socket_addrs.as_slice()).await {
                            Err(e) => {
                                return Err(anyhow!(PpaassError::IoError { source: e }));
                            },
                            Ok(v) => v,
                        };
                        let connection_id = generate_uuid();
                        let tcp_initialize_success_message_payload =
                            PpaassMessagePayload::new(Some(connection_id), source_address, Some(target_address), payload_type, data);
                        let payload_bytes: Vec<u8> = tcp_initialize_success_message_payload.try_into()?;
                        let tcp_initialize_success_message = PpaassMessage::new(
                            user_token,
                            ppaass_protocol::PpaassMessagePayloadEncryption::Aes("".as_bytes().to_vec()),
                            payload_bytes,
                        );
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpRelay) => {},
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpDestory) => {},
                    invalid_type => {
                        error!("Fail to parse agent payload type because of receove invalid data: {invalid_type:?}");
                        return Err(anyhow!(PpaassError::CodecError));
                    },
                }
            }
        });

        Ok(())
    }
}
