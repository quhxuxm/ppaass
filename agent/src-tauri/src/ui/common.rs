use serde::{Deserialize, Serialize};

use crate::{config::AgentConfig, server::AgentServerHandler};

pub(crate) struct UiState {
    pub agent_server_handler: AgentServerHandler,
    pub configuration: AgentConfig,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct UiServerStatus {
    pub status: String,
    pub timestamp: Option<i64>,
}
