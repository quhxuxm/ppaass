use anyhow::Result;
use futures::TryStreamExt;
use ppaass_common::generate_uuid;
use ppaass_io::PpaassTcpConnection;
use tokio::net::TcpStream;

use crate::crypto::ProxyServerRsaCryptoFetcher;

#[derive(Debug)]
pub(crate) struct ProxyTcpTunnel {
    id: String,
    agent_tcp_connection: PpaassTcpConnection<TcpStream, ProxyServerRsaCryptoFetcher>,
    target_tcp_stream: Option<TcpStream>,
}

impl ProxyTcpTunnel {
    pub(crate) fn new(agent_tcp_connection: PpaassTcpConnection<TcpStream, ProxyServerRsaCryptoFetcher>) -> Self {
        Self {
            id: generate_uuid(),
            agent_tcp_connection,
            target_tcp_stream: None,
        }
    }

    pub(crate) fn get_id(&self) -> &str {
        &self.id
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let mut agent_tcp_connection = self.agent_tcp_connection;
        let agent_message = agent_tcp_connection.try_next().await?;
        Ok(())
    }
}
