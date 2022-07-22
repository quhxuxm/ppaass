use crate::{config::AgentConfig, server::AgentServerHandler};

pub(crate) struct UiState {
    pub agent_server_handler: AgentServerHandler,
    pub configuration: AgentConfig,
}
