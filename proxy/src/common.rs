use ppaass_io::PpaassMessageFramed;
use tokio::net::TcpStream;

use crate::crypto::ProxyServerRsaCryptoFetcher;

pub(crate) type AgentMessageFramed = PpaassMessageFramed<TcpStream, ProxyServerRsaCryptoFetcher>;
