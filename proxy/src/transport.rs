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
    TcpInitialize {
        source_address: Option<PpaassProtocolAddress>,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    TcpReplay {
        data: Vec<u8>,
        source_address: Option<PpaassProtocolAddress>,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    TcpDestory {
        source_address: Option<PpaassProtocolAddress>,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    ConnectionKeepAlive {
        user_token: String,
    },
    DomainNameResolve {
        data: Vec<u8>,
        user_token: String,
    },
}

#[derive(Debug)]
struct AgentToTargetData {
    data_type: AgentToTargetDataType,
}

#[derive(Debug)]
enum TargetToAgentDataType {
    TcpInitializeSuccess {
        source_address: Option<PpaassProtocolAddress>,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    TcpInitializeFail {
        source_address: Option<PpaassProtocolAddress>,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    TcpReplaySuccess {
        source_address: Option<PpaassProtocolAddress>,
        target_address: PpaassProtocolAddress,
        user_token: String,
        data: Vec<u8>,
    },
    TcpReplayFail {
        source_address: Option<PpaassProtocolAddress>,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    TcpDestorySuccess {
        source_address: PpaassProtocolAddress,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    TcpDestoryFail {
        source_address: PpaassProtocolAddress,
        target_address: PpaassProtocolAddress,
        user_token: String,
    },
    ConnectionKeepAliveSuccess {
        user_token: String,
    },
    ConnectionKeepAliveFail {
        user_token: String,
    },
    DomainNameResolveSuccess {
        user_token: String,
        data: Vec<u8>,
    },
    DomainNameResolveFail {
        user_token: String,
    },
}

#[derive(Debug)]
struct TargetToAgentData {
    data_type: TargetToAgentDataType,
}

#[derive(Debug)]
pub(crate) struct Transport {
    id: String,
    agent_edge: Option<AgentEdge>,
    target_edge: Option<TargetEdge>,
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
        let target_edge = TargetEdge::new(id.clone(), agent_to_target_receiver, target_to_agent_sender, connection_number_permit);
        Self {
            id,
            agent_edge: Some(agent_edge),
            target_edge: Some(target_edge),
        }
    }

    pub(crate) fn get_id(&self) -> &str {
        &self.id
    }

    pub(crate) async fn exec(&mut self) {
        debug!("Begin to execute transport [{}]", self.id);
        let mut agent_edge = self.agent_edge.take().unwrap();
        let mut target_edge = self.target_edge.take().unwrap();
        tokio::spawn(async move { agent_edge.exec().await });
        tokio::spawn(async move { target_edge.exec().await });
    }
}
