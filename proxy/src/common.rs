use ppaass_io::PpaassMessageFramed;
use ppaass_protocol::PpaassMessagePayloadEncryptionSelector;
use tokio::net::TcpStream;

use crate::crypto::ProxyServerRsaCryptoFetcher;

pub(crate) type AgentMessageFramed = PpaassMessageFramed<TcpStream, ProxyServerRsaCryptoFetcher>;

pub(crate) struct ProxyServerPayloadEncryptionSelector {}

impl PpaassMessagePayloadEncryptionSelector for ProxyServerPayloadEncryptionSelector {}
