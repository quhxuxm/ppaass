use ppaass_common::PpaassMessagePayloadEncryptionSelector;

pub mod config;
pub mod server;

pub(crate) mod crypto;
pub(crate) mod error;
pub(crate) mod pool;
pub(crate) mod processor;

pub const SOCKS_V5: u8 = 5;
pub const SOCKS_V4: u8 = 4;

pub struct AgentServerPayloadEncryptionTypeSelector;

impl PpaassMessagePayloadEncryptionSelector for AgentServerPayloadEncryptionTypeSelector {}
