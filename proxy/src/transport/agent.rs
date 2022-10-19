use std::sync::Arc;

use anyhow::anyhow;

use futures::{SinkExt, StreamExt, TryStreamExt};
use ppaass_common::{generate_uuid, PpaassError};

use ppaass_protocol::{
    PpaassMessage, PpaassMessageAgentPayloadTypeValue, PpaassMessageParts, PpaassMessagePayload, PpaassMessagePayloadParts, PpaassMessagePayloadType,
};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tracing::{error, info};

use crate::{common::AgentMessageFramed, config::ProxyServerConfig};

use super::{AgentToTargetData, TargetToAgentData};

#[derive(Debug)]
pub(crate) struct AgentEdge {
    transport_id: String,
    agent_message_framed: AgentMessageFramed,
    agent_to_target_data_sender: Sender<AgentToTargetData>,
    target_to_agent_data_receiver: Receiver<TargetToAgentData>,
}

impl AgentEdge {
    pub(crate) fn new(
        transport_id: String, agent_message_framed: AgentMessageFramed, _configuration: Arc<ProxyServerConfig>,
        agent_to_target_data_sender: Sender<AgentToTargetData>, target_to_agent_data_receiver: Receiver<TargetToAgentData>,
    ) -> Self {
        Self {
            transport_id,
            agent_message_framed,
            agent_to_target_data_sender,
            target_to_agent_data_receiver,
        }
    }

    pub(crate) async fn exec(self) {
        let (mut agent_message_sink, mut agent_message_stream) = self.agent_message_framed.split();
        let agent_to_target_data_sender = self.agent_to_target_data_sender;
        let mut target_to_agent_data_receiver = self.target_to_agent_data_receiver;
        let transport_id = self.transport_id.clone();
        tokio::spawn(async move {
            loop {
                let agent_message = agent_message_stream.try_next().await?;
                let PpaassMessageParts { payload_bytes, user_token, .. } = match agent_message {
                    None => {
                        info!("Transport [{transport_id}] agent edge disconnected.");
                        drop(agent_to_target_data_sender);
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
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::DomainNameResolve) => {
                        if let Err(e) = agent_to_target_data_sender
                            .send(AgentToTargetData {
                                data_type: super::AgentToTargetDataType::DomainNameResolve { data },
                            })
                            .await
                        {
                            error!("Fail to send domain name resolve to target because of sender error: {e:?}");
                        };
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::ConnectionKeepAlive) => {
                        if let Err(e) = agent_to_target_data_sender
                            .send(AgentToTargetData {
                                data_type: super::AgentToTargetDataType::ConnectionKeepAlive,
                            })
                            .await
                        {
                            error!("Fail to send connection keep alive to target because of sender error: {e:?}");
                        };
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize) => {
                        let target_address = target_address.ok_or(anyhow!("No target address assigned."))?;
                        if let Err(e) = agent_to_target_data_sender
                            .send(AgentToTargetData {
                                data_type: super::AgentToTargetDataType::TcpInitialize { target_address },
                            })
                            .await
                        {
                            error!("Fail to send tcp initialize to target because of sender error: {e:?}");
                        };
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpRelay) => {
                        if let Err(e) = agent_to_target_data_sender
                            .send(AgentToTargetData {
                                data_type: super::AgentToTargetDataType::TcpReplay { data },
                            })
                            .await
                        {
                            error!("Fail to send tcp relay data to target because of sender error: {e:?}");
                        };
                        continue;
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpDestory) => {
                        if let Err(e) = agent_to_target_data_sender
                            .send(AgentToTargetData {
                                data_type: super::AgentToTargetDataType::TcpDestory,
                            })
                            .await
                        {
                            error!("Fail to send tcp destory to target because of sender error: {e:?}");
                        };
                        continue;
                    },
                    invalid_type => {
                        error!("Fail to parse agent payload type because of receove invalid data: {invalid_type:?}");
                        return Err(anyhow!(PpaassError::CodecError));
                    },
                }
            }
        });
        let transport_id = self.transport_id.clone();
        tokio::spawn(async move {
            loop {
                let target_to_agent_data = match target_to_agent_data_receiver.recv().await {
                    None => {
                        info!("Transport [{transport_id}] target edge disconnected.");
                        return;
                    },
                    Some(v) => v,
                };
            }
        });
    }
}
