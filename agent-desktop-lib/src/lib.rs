use crypto::AgentServerRsaCryptoFetcher;
use futures::stream::{SplitSink, SplitStream};
use ppaass_io::PpaassMessageFramed;
use ppaass_protocol::{PpaassMessage, PpaassMessagePayloadEncryptionSelector};
use tokio::net::TcpStream;

pub mod config;
pub(crate) mod crypto;
pub(crate) mod flow;
pub(crate) mod pool;
pub mod server;

pub(crate) type PpaassMessageFramedWrite<'a> = SplitSink<&'a mut PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>, PpaassMessage>;

pub(crate) type PpaassMessageFramedRead<'a> = SplitStream<&'a mut PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>>;

pub struct AgentServerPayloadEncryptionTypeSelector;

impl PpaassMessagePayloadEncryptionSelector for AgentServerPayloadEncryptionTypeSelector {}
