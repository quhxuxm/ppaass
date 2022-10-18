use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};

use futures::{SinkExt, StreamExt, TryStreamExt};
use ppaass_common::{generate_uuid, PpaassError};

use ppaass_protocol::{
    PpaassMessage, PpaassMessageAgentPayloadTypeValue, PpaassMessageParts, PpaassMessagePayload, PpaassMessagePayloadParts, PpaassMessagePayloadType,
};
use tokio::sync::{
    mpsc::{channel, Sender},
    Mutex,
};
use tracing::{error, info};

use crate::{common::AgentMessageFramed, config::ProxyServerConfig};

use super::{target::TargetTcpTransport, TargetTcpTransportInput, TargetTcpTransportInputType, TargetTcpTransportOutput};

#[derive(Debug)]
pub(crate) struct AgentTcpTransport {
    id: String,
    agent_message_framed: AgentMessageFramed,
    target_tcp_transport_input_sender_repository: Mutex<HashMap<String, Sender<TargetTcpTransportInput>>>,
}

impl AgentTcpTransport {
    pub(crate) fn new(
        agent_message_framed: AgentMessageFramed, target_tcp_transport_input_sender_repository: Mutex<HashMap<String, Sender<TargetTcpTransportInput>>>,
        _configuration: Arc<ProxyServerConfig>,
    ) -> Self {
        let (outbound_sender, outbound_receiver) = channel::<TargetTcpTransportOutput>(1024);
        Self {
            id: generate_uuid(),
            agent_message_framed,
            target_tcp_transport_input_sender_repository,
        }
    }

    pub(crate) fn get_id(&self) -> &str {
        &self.id
    }

    pub(crate) async fn exec(self) -> Result<()> {
        let (mut agent_message_sink, mut agent_message_stream) = self.agent_message_framed.split();

        let id = self.id;
        tokio::spawn(async move {
            loop {
                let agent_message = agent_message_stream.try_next().await?;
                let PpaassMessageParts { payload_bytes, user_token, .. } = match agent_message {
                    None => {
                        info!("Agent tcp transport [{id}] disconnected.");
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
                        if let Err(e) = agent_message_sink.send(keep_alive_success_message).await {
                            error!("Fail to do keep alive for agent tcp transport [{id}], error: {e:?}");
                            return Err(anyhow!(e));
                        };
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize) => {
                        let target_address = target_address.ok_or(anyhow!("No target address assigned."))?;
                        let (target_tcp_transport_input_sender, target_tcp_transport_input_receiver) = channel::<TargetTcpTransportInput>(1024);
                        let (target_tcp_transport_output_sender, target_tcp_transport_output_receiver) = channel::<TargetTcpTransportOutput>(1024);
                        let target_tcp_transport = TargetTcpTransport::new(target_tcp_transport_input_receiver, target_tcp_transport_output_sender);
                        target_tcp_transport.exec().await?;
                        let target_tcp_transport_input = TargetTcpTransportInput::new(TargetTcpTransportInputType::Connect { target_address });
                        target_tcp_transport_input_sender
                            .send(target_tcp_transport_input)
                            .await
                            .map_err(|e| anyhow!("Can not send input to target transport"))?;
                        let mut target_tcp_transport_input_sender_repository = self.target_tcp_transport_input_sender_repository.lock().await;
                        target_tcp_transport_input_sender_repository.insert(target_tcp_transport.get_id().to_owned(), target_tcp_transport_input_sender);
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpRelay) => {
                        let connection_id = connection_id.ok_or(anyhow!("No connection id assigned."))?;
                        let target_tcp_transport_input_sender = self.target_tcp_transport_input_sender_repository.get(&connection_id);
                        let target_tcp_transport_input_sender =
                            target_tcp_transport_input_sender.ok_or(anyhow!("Can not find target tcp transport input sender."))?;

                        let target_tcp_transport_input = TargetTcpTransportInput::new(TargetTcpTransportInputType::Relay { data });
                        target_tcp_transport_input_sender
                            .send(target_tcp_transport_input)
                            .await
                            .map_err(|e| anyhow!("Can not send input to target transport"))?;
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpDestory) => {
                        let connection_id = connection_id.ok_or(anyhow!("No connection id assigned."))?;
                        let mut target_tcp_transport_input_sender_repository = self.target_tcp_transport_input_sender_repository.lock().await;
                        let target_tcp_transport_input_sender = target_tcp_transport_input_sender_repository.remove(&connection_id);
                        target_tcp_transport_input_sender.ok_or(anyhow!("Can not find target tcp transport input sender."))?;
                        drop(target_tcp_transport_input_sender);
                        continue;
                    },
                    invalid_type => {
                        error!("Fail to parse agent payload type because of receove invalid data: {invalid_type:?}");
                        return Err(anyhow!(PpaassError::CodecError));
                    },
                }
            }
        });

        tokio::spawn(async move {});

        Ok(())
    }
}
