use std::collections::HashMap;

use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use ppaass_common::{generate_uuid, PpaassError};
use ppaass_io::PpaassTcpConnection;
use ppaass_protocol::{PpaassMessageAgentPayloadTypeValue, PpaassMessageParts, PpaassMessagePayload, PpaassMessagePayloadParts, PpaassMessagePayloadType};
use tokio::net::TcpStream;
use tracing::error;

use crate::crypto::ProxyServerRsaCryptoFetcher;

#[derive(Debug)]
pub(crate) struct ProxyTcpTunnel {
    tunnel_id: String,
    agent_tcp_connection: PpaassTcpConnection<TcpStream, ProxyServerRsaCryptoFetcher>,
    target_tcp_stream_repository: HashMap<String, TcpStream>,
}

impl ProxyTcpTunnel {
    pub(crate) fn new(agent_tcp_connection: PpaassTcpConnection<TcpStream, ProxyServerRsaCryptoFetcher>) -> Self {
        Self {
            tunnel_id: generate_uuid(),
            agent_tcp_connection,
            target_tcp_stream_repository: HashMap::new(),
        }
    }

    pub(crate) fn get_tunnel_id(&self) -> &str {
        &&self.tunnel_id
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let mut agent_tcp_connection = self.agent_tcp_connection;
        let agent_message = agent_tcp_connection.try_next().await?;
        let PpaassMessageParts { payload_bytes, .. } = match agent_message {
            None => {
                return Ok(());
            },
            Some(v) => v.split(),
        };
        let agent_message_payload: PpaassMessagePayload = payload_bytes.try_into()?;
        let PpaassMessagePayloadParts {
            connection_id: client_connection_id,
            payload_type,
            source_address,
            target_address,
            additional_info,
            data,
        } = agent_message_payload.split();
        match payload_type {
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::DomainNameResolve) => {},
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::IdleConnectionKeepAlive) => {},
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize) => {},
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpRelay) => {},
            PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpDestory) => {},
            invalid_type => {
                error!("Fail to parse agent payload type because of receove invalid data: {invalid_type:?}");
                return Err(anyhow!(PpaassError::CodecError));
            },
        }
        Ok(())
    }
}
