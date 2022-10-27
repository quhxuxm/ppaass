use std::{sync::Arc, vec};

use futures::{SinkExt, StreamExt, TryStreamExt};
use ppaass_common::generate_uuid;

use ppaass_protocol::{
    PayloadAdditionalInfoKey, PayloadAdditionalInfoValue, PpaassMessage, PpaassMessageAgentPayloadTypeValue, PpaassMessageParts, PpaassMessagePayload,
    PpaassMessagePayloadEncryptionSelector, PpaassMessagePayloadParts, PpaassMessagePayloadType, PpaassMessageProxyPayloadTypeValue,
};

use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{debug, error, info};

use crate::{
    common::{AgentMessageFramed, ProxyServerPayloadEncryptionSelector},
    config::ProxyServerConfig,
};

use super::{AgentToTargetData, AgentToTargetDataType, TargetToAgentData, TargetToAgentDataType};

#[derive(Debug)]
pub(super) struct AgentEdge {
    transport_id: String,
    agent_message_framed: AgentMessageFramed,
    agent_to_target_data_sender: Sender<AgentToTargetData>,
    target_to_agent_data_receiver: Receiver<TargetToAgentData>,
}

impl AgentEdge {
    pub(super) fn new(
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

    pub(super) async fn exec(self) {
        let agent_message_framed = self.agent_message_framed;
        let (mut agent_message_sink, mut agent_message_stream) = agent_message_framed.split();
        let agent_to_target_data_sender = self.agent_to_target_data_sender;
        let mut target_to_agent_data_receiver = self.target_to_agent_data_receiver;
        let transport_id = self.transport_id.clone();
        tokio::spawn(async move {
            loop {
                let agent_message = match agent_message_stream.try_next().await {
                    Err(e) => {
                        error!("Fail to send agent to target data because of error: {e:?}");
                        return;
                    },
                    Ok(v) => v,
                };
                let PpaassMessageParts {
                    payload_bytes,
                    user_token,
                    id: agent_message_id,
                    ..
                } = match agent_message {
                    None => {
                        info!("Transport [{transport_id}] agent edge disconnected.");
                        drop(agent_to_target_data_sender);
                        return;
                    },
                    Some(v) => v.split(),
                };
                let agent_message_payload: PpaassMessagePayload = match payload_bytes.try_into() {
                    Err(e) => {
                        error!("Fail to send agent to target data because of error: {e:?}");
                        return;
                    },
                    Ok(v) => v,
                };
                let PpaassMessagePayloadParts {
                    payload_type,
                    source_address,
                    target_address,
                    additional_info,
                    data,
                } = agent_message_payload.split();
                let reference_message_id_value = additional_info.get(&PayloadAdditionalInfoKey::ReferenceMessageId);
                if let Some(value) = reference_message_id_value {
                    match value {
                        PayloadAdditionalInfoValue::ReferenceMessageIdValue(reference_message_id) => {
                            debug!("Receive agent message [{agent_message_id}], reference messag [{reference_message_id}]")
                        },
                    }
                }
                let agent_to_target_data = match payload_type {
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::DomainNameResolve) => AgentToTargetData {
                        data_type: AgentToTargetDataType::DomainNameResolve { data, user_token },
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::ConnectionKeepAlive) => AgentToTargetData {
                        data_type: AgentToTargetDataType::ConnectionKeepAlive { user_token },
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize) => {
                        let target_address = match target_address {
                            None => {
                                error!("Fail to send agent to target data.");
                                return;
                            },
                            Some(v) => v,
                        };
                        AgentToTargetData {
                            data_type: AgentToTargetDataType::TcpInitialize {
                                source_address,
                                target_address,
                                user_token,
                            },
                        }
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpRelay) => {
                        let target_address = match target_address {
                            None => {
                                error!("Fail to send agent to target data.");
                                return;
                            },
                            Some(v) => v,
                        };
                        AgentToTargetData {
                            data_type: AgentToTargetDataType::TcpReplay {
                                source_address,
                                target_address,
                                user_token,
                                data,
                            },
                        }
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpDestory) => AgentToTargetData {
                        data_type: AgentToTargetDataType::TcpDestory,
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpInitialize) => {
                        let target_address = match target_address {
                            None => {
                                error!("Fail to send agent to target data.");
                                return;
                            },
                            Some(v) => v,
                        };
                        AgentToTargetData {
                            data_type: AgentToTargetDataType::UdpInitialize {
                                source_address,
                                target_address,
                                user_token,
                            },
                        }
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpRelay) => {
                        let target_address = match target_address {
                            None => {
                                error!("Fail to send agent to target data.");
                                return;
                            },
                            Some(v) => v,
                        };
                        AgentToTargetData {
                            data_type: AgentToTargetDataType::UdpReplay {
                                data,
                                source_address,
                                target_address,
                                user_token,
                            },
                        }
                    },
                    PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpDestory) => AgentToTargetData {
                        data_type: AgentToTargetDataType::UdpDestory,
                    },
                    invalid_type => {
                        error!("Fail to parse agent payload type because of receove invalid data: {invalid_type:?}");
                        return;
                    },
                };
                if let Err(e) = agent_to_target_data_sender.send(agent_to_target_data).await {
                    error!("Fail to send agent to target data because of sender error: {e:?}");
                    return;
                };
            }
        });
        let transport_id = self.transport_id.clone();
        tokio::spawn(async move {
            loop {
                let TargetToAgentData { data_type } = match target_to_agent_data_receiver.recv().await {
                    None => {
                        info!("Transport [{transport_id}] target edge disconnected.");
                        return;
                    },
                    Some(v) => v,
                };
                let (message_payload, user_token) = match data_type {
                    TargetToAgentDataType::TcpInitializeSuccess {
                        target_address,
                        source_address,
                        user_token,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpInitializeSuccess),
                            vec![],
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::TcpInitializeFail {
                        target_address,
                        source_address,
                        user_token,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpInitializeFail),
                            vec![],
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::TcpReplaySuccess {
                        data,
                        source_address,
                        target_address,
                        user_token,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpRelaySuccess),
                            data,
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::TcpReplayFail {
                        source_address,
                        target_address,
                        user_token,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::TcpRelayFail),
                            vec![],
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::ConnectionKeepAliveSuccess { user_token } => (
                        PpaassMessagePayload::new(
                            None,
                            None,
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::ConnectionKeepAliveSuccess),
                            vec![],
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::DomainNameResolveSuccess { data, user_token } => (
                        PpaassMessagePayload::new(
                            None,
                            None,
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::DomainNameResolveSuccess),
                            data,
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::DomainNameResolveFail { user_token } => (
                        PpaassMessagePayload::new(
                            None,
                            None,
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::DomainNameResolveFail),
                            vec![],
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::UdpInitializeSuccess {
                        source_address,
                        target_address,
                        user_token,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::UdpInitializeSuccess),
                            vec![],
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::UdpInitializeFail {
                        source_address,
                        target_address,
                        user_token,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::UdpInitializeFail),
                            vec![],
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::UdpReplaySuccess {
                        source_address,
                        target_address,
                        user_token,
                        data,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::UdpRelaySuccess),
                            data,
                        ),
                        user_token,
                    ),
                    TargetToAgentDataType::UdpReplayFail {
                        source_address,
                        target_address,
                        user_token,
                    } => (
                        PpaassMessagePayload::new(
                            source_address,
                            Some(target_address),
                            PpaassMessagePayloadType::ProxyPayload(PpaassMessageProxyPayloadTypeValue::UdpRelayFail),
                            vec![],
                        ),
                        user_token,
                    ),
                };
                let message_payload_bytes = match message_payload.try_into() {
                    Err(e) => {
                        error!("Transport [{transport_id}] fail to initialize tcp connection because of error: {e:?}.");
                        return;
                    },
                    Ok(v) => v,
                };
                let message = PpaassMessage::new(
                    &user_token,
                    ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes())),
                    message_payload_bytes,
                );
                if let Err(e) = agent_message_sink.send(message).await {
                    error!("Transport [{transport_id}] fail to send message to agent because of error: {e:?}.");
                    return;
                };
            }
        });
    }
}
