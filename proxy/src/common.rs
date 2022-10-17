use ppaass_io::PpaassTcpConnection;
use tokio::net::TcpStream;

use crate::crypto::ProxyServerRsaCryptoFetcher;

pub(crate) type AgentTcpConnection = PpaassTcpConnection<TcpStream, ProxyServerRsaCryptoFetcher>;
