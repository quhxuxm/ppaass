use ppaass_common::PpaassMessagePayloadEncryptionSelector;

pub mod config;
pub(crate) mod crypto;
pub(crate) mod flow;
pub(crate) mod pool;
pub mod server;

pub const SOCKS_V5: u8 = 5;
pub const SOCKS_V4: u8 = 4;

pub struct AgentServerPayloadEncryptionTypeSelector;

impl PpaassMessagePayloadEncryptionSelector for AgentServerPayloadEncryptionTypeSelector {}
