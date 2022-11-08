pub(crate) mod agent;
pub(crate) mod target;

use std::sync::Arc;

use ppaass_common::generate_uuid;

use ppaass_protocol::PpaassNetAddress;
use tokio::sync::{mpsc::channel, OwnedSemaphorePermit};
use tracing::debug;

use crate::{common::AgentMessageFramed, config::ProxyServerConfig};

#[derive(Debug)]
pub(crate) struct Tunnel {
    id: String,
    agent_edge: AgentEdge,
    target_edge: TargetEdge,
}

impl Tunnel {
    pub(crate) fn new(agent_message_framed: AgentMessageFramed, configuration: Arc<ProxyServerConfig>) -> Self {
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
        Self { id, agent_edge, target_edge }
    }

    pub(crate) async fn exec(self) {
        debug!("Begin to execute transport [{}]", self.id);
        let agent_edge = self.agent_edge;
        let target_edge = self.target_edge;
        tokio::spawn(async move { agent_edge.exec().await });
        tokio::spawn(async move { target_edge.exec().await });
    }
}
