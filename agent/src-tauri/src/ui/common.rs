use crate::{
    config::{AgentArguments, AgentConfig},
    server::AgentServerHandler,
};

pub(crate) struct UiState {
    pub agent_server_handler: AgentServerHandler,
    pub configuration: AgentConfig,
    pub arguments: AgentArguments,
}
