use ppaass_common::PpaassMessagePayloadEncryptionSelector;

pub mod config;
pub(crate) mod crypto;
pub(crate) mod flow;
pub(crate) mod pool;
pub mod server;

pub struct AgentServerPayloadEncryptionTypeSelector;

impl PpaassMessagePayloadEncryptionSelector for AgentServerPayloadEncryptionTypeSelector {}
