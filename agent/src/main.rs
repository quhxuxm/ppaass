pub(crate) mod config;
pub(crate) mod server;

pub(crate) mod connection;
pub(crate) mod crypto;
pub(crate) mod error;
pub(crate) mod processor;

use anyhow::Result;

use config::AGENT_CONFIG;
use ppaass_common::PpaassMessagePayloadEncryptionSelector;

use log::error;
use server::AgentServer;
use tokio::runtime::Builder;

pub const SOCKS_V5: u8 = 5;
pub const SOCKS_V4: u8 = 4;

pub struct AgentServerPayloadEncryptionTypeSelector;

impl PpaassMessagePayloadEncryptionSelector for AgentServerPayloadEncryptionTypeSelector {}

fn main() -> Result<()> {
    log4rs::init_file("resources/config/ppaass-agent-log.yml", Default::default())?;

    let agent_server_runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(AGENT_CONFIG.get_worker_thread_number())
        .build()?;
    agent_server_runtime.block_on(async move {
        let mut agent_server = AgentServer::default();
        if let Err(e) = agent_server.start().await {
            error!("Fail to start agent server because of error: {e:?}");
        };
    });
    Ok(())
}
