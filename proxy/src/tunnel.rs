use std::{
    collections::HashMap,
    net::{SocketAddr, ToSocketAddrs},
};

use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use ppaass_common::{generate_uuid, PpaassError};
use ppaass_io::PpaassTcpConnection;
use ppaass_protocol::{
    PpaassMessage, PpaassMessageAgentPayloadTypeValue, PpaassMessageParts, PpaassMessagePayload, PpaassMessagePayloadParts, PpaassMessagePayloadType,
};
use tokio::net::TcpStream;
use tracing::error;

use crate::crypto::ProxyServerRsaCryptoFetcher;

mod repository;
pub(crate) use repository::*;

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

    pub(crate) async fn exec(&mut self) -> Result<()> {
        let agent_tcp_connection = &mut self.agent_tcp_connection;
        loop {
            let agent_message = agent_tcp_connection.try_next().await?;
            let PpaassMessageParts { payload_bytes, user_token, .. } = match agent_message {
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
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::ConnectionKeepAlive) => {},
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize) => {
                    let target_address = target_address.ok_or(PpaassError::CodecError)?;
                    let target_socket_addrs = target_address.to_socket_addrs()?;
                    let target_socket_addrs = target_socket_addrs.collect::<Vec<SocketAddr>>();
                    let target_tcp_stream = match TcpStream::connect(target_socket_addrs.as_slice()).await {
                        Err(e) => {
                            return Err(anyhow!(PpaassError::IoError { source: e }));
                        },
                        Ok(v) => v,
                    };
                    let connection_id = generate_uuid();
                    self.target_tcp_stream_repository.insert(connection_id.clone(), target_tcp_stream);
                    let tcp_initialize_success_message_payload =
                        PpaassMessagePayload::new(Some(connection_id), source_address, Some(target_address), payload_type, data);
                    let payload_bytes: Vec<u8> = tcp_initialize_success_message_payload.try_into()?;
                    let tcp_initialize_success_message = PpaassMessage::new(
                        user_token,
                        ppaass_protocol::PpaassMessagePayloadEncryption::Aes("".as_bytes().to_vec()),
                        payload_bytes,
                    );
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpRelay) => {},
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpDestory) => {},
                invalid_type => {
                    error!("Fail to parse agent payload type because of receove invalid data: {invalid_type:?}");
                    return Err(anyhow!(PpaassError::CodecError));
                },
            }
        }

        Ok(())
    }
}

unsafe impl Sync for ProxyTcpTunnel {}

unsafe impl Send for ProxyTcpTunnel {}
