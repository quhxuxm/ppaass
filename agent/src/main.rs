mod common;

pub mod config;
pub mod server;

pub(crate) mod crypto;
pub(crate) mod error;
pub(crate) mod pool;
pub(crate) mod processor;

use ppaass_common::PpaassMessagePayloadEncryptionSelector;

use anyhow::Result;
use log::error;
use server::AgentServer;
use tokio::runtime::Builder;

pub const SOCKS_V5: u8 = 5;
pub const SOCKS_V4: u8 = 4;

pub struct AgentServerPayloadEncryptionTypeSelector;

impl PpaassMessagePayloadEncryptionSelector for AgentServerPayloadEncryptionTypeSelector {}

fn main() -> Result<()> {
    log4rs::init_file("resources/config/ppaass-agent-log.yaml", Default::default())?;
    let agent_server_runtime = Builder::new_multi_thread().enable_all().worker_threads(64).build()?;
    agent_server_runtime.block_on(async move {
        let mut agent_server = AgentServer::default();
        if let Err(e) = agent_server.start().await {
            error!("Fail to start agent server because of error: {e:?}");
        };
    });
    Ok(())
}
