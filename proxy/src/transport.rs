pub(crate) mod agent;
pub(crate) mod target;

use std::sync::Arc;

use ppaass_common::generate_uuid;

use ppaass_protocol::PpaassProtocolAddress;
use tokio::sync::{mpsc::channel, OwnedSemaphorePermit};
use tracing::debug;

use crate::{common::AgentMessageFramed, config::ProxyServerConfig};

use self::{agent::AgentEdge, target::TargetEdge};

#[derive(Debug)]
enum AgentToTargetDataType {
    TcpInitialize { target_address: PpaassProtocolAddress },
    TcpReplay { data: Vec<u8> },
    TcpDestory,
    ConnectionKeepAlive,
    DomainNameResolve { data: Vec<u8> },
}

#[derive(Debug)]
struct AgentToTargetData {
    data_type: AgentToTargetDataType,
}

#[derive(Debug)]
enum TargetToAgentDataType {
    TcpInitializeSuccess { target_address: PpaassProtocolAddress },
    TcpInitializeFail { target_address: PpaassProtocolAddress },
    TcpReplaySuccess { data: Vec<u8> },
    TcpReplayFail { data: Vec<u8> },
    TcpDestorySuccess,
    TcpDestoryFail,
    ConnectionKeepAliveSuccess,
    ConnectionKeepAliveFail,
    DomainNameResolveSuccess { data: Vec<u8> },
    DomainNameResolveFail { data: Vec<u8> },
}

#[derive(Debug)]
struct TargetToAgentData {
    data_type: TargetToAgentDataType,
}

#[derive(Debug)]
pub(crate) struct Transport {
    id: String,
    agent_edge: AgentEdge,
    target_edge: TargetEdge,
    connection_number_permit: OwnedSemaphorePermit,
}

impl Transport {
    pub(crate) fn new(agent_message_framed: AgentMessageFramed, configuration: Arc<ProxyServerConfig>, connection_number_permit: OwnedSemaphorePermit) -> Self {
        let id = generate_uuid();
        let (agent_to_target_sender, agent_to_target_receiver) = channel::<AgentToTargetData>(1024);
        let (target_to_agent_sender, target_to_agent_receiver) = channel::<TargetToAgentData>(1024);
        let agent_edge = AgentEdge::new(
            id.clone(),
            agent_message_framed,
            configuration,
            agent_to_target_sender,
            target_to_agent_receiver,
        );
        let target_edge = TargetEdge::new(id.clone(), agent_to_target_receiver, target_to_agent_sender);
        Self {
            id,
            agent_edge,
            target_edge,
            connection_number_permit,
        }
    }

    pub(crate) fn get_id(&self) -> &str {
        &self.id
    }

    pub(crate) async fn exec(self) {
        debug!("Begin to execute transport [{}]", self.id);
        let agent_edge_guard = tokio::spawn(self.agent_edge.exec());
        let target_edge_guard = tokio::spawn(self.target_edge.exec());
        tokio::join!(agent_edge_guard, target_edge_guard);
        drop(self.connection_number_permit)
    }
}
