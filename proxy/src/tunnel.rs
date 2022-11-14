use std::collections::HashMap;

use std::net::IpAddr;
use std::{net::SocketAddr, sync::Arc};

use futures::{SinkExt, StreamExt};
use ppaass_common::generate_uuid;
use tokio::sync::Mutex;

use crate::common::ProxyServerPayloadEncryptionSelector;
use crate::tunnel::tcp_session::TcpSession;
use crate::{common::AgentMessageFramed, config::ProxyServerConfig};
use anyhow::Result;
use ppaass_protocol::tcp_destroy::TcpDestroyRequestPayload;
use ppaass_protocol::tcp_initialize::TcpInitializeRequestPayload;
use ppaass_protocol::tcp_relay::TcpRelayPayload;
use ppaass_protocol::{
    domain_resolve::DomainResolveRequestPayload, heartbeat::HeartbeatRequestPayload, MessageUtil, PpaassMessageAgentPayloadTypeValue, PpaassMessageParts,
    PpaassMessagePayload, PpaassMessagePayloadEncryptionSelector, PpaassMessagePayloadParts, PpaassMessagePayloadType, PpaassNetAddress,
};
use tracing::{debug, error, trace};

mod tcp_session;

#[derive(Debug)]
pub(crate) struct TcpTunnel {
    id: String,
    agent_message_framed: AgentMessageFramed,
    agent_socket_address: SocketAddr,
    configuration: Arc<ProxyServerConfig>,
    tcp_session_container: HashMap<String, TcpSession>,
}

impl TcpTunnel {
    pub(crate) fn new(agent_message_framed: AgentMessageFramed, agent_socket_address: SocketAddr, configuration: Arc<ProxyServerConfig>) -> Self {
        Self {
            id: generate_uuid(),
            agent_message_framed,
            agent_socket_address,
            configuration,
            tcp_session_container: HashMap::new(),
        }
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let tunnel_id = self.id;
        let agent_socket_address = self.agent_socket_address;
        debug!("tunnel [{tunnel_id}] begin for agent connection: {agent_socket_address}");
        let agent_message_framed = self.agent_message_framed;
        let (agent_message_framed_write, mut agent_message_framed_read) = agent_message_framed.split();
        let agent_message_framed_write = Arc::new(Mutex::new(agent_message_framed_write));
        while let Some(agent_message) = agent_message_framed_read.next().await {
            let agent_message = match agent_message {
                Err(e) => {
                    error!("Fail to read agent message because of error: {e:?}");
                    return Err(e);
                },
                Ok(v) => v,
            };
            let PpaassMessageParts {
                id: agent_message_id,
                user_token,
                payload_bytes,
                ..
            } = agent_message.split();
            let PpaassMessagePayloadParts {
                payload_type: agent_message_payload_type,
                data: agent_message_payload_data,
            } = TryInto::<PpaassMessagePayload>::try_into(payload_bytes)?.split();
            debug!("Receive agent message: {agent_message_id}, payload_type: {agent_message_payload_type:?}");

            match agent_message_payload_type {
                PpaassMessagePayloadType::ProxyPayload(v) => {
                    error!("Fail to handle agent message because of invalid payload type: {v:?}");
                    break;
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::Heartbeat) => {
                    let agent_message_framed_write = agent_message_framed_write.clone();
                    let user_token = user_token.clone();
                    tokio::spawn(async move {
                        let heartbeat_request: HeartbeatRequestPayload = agent_message_payload_data.try_into()?;
                        let src_address = heartbeat_request.src_address;
                        let dest_address = heartbeat_request.dest_address;
                        trace!("Receive agent heartbeat message, agent address: {agent_socket_address}, source address: {src_address:?}, target address: {dest_address:?}");
                        let heartbeat_response_success_payload_encryption =
                            ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                        let heartbeat_response_success = MessageUtil::create_proxy_heartbeat_response(
                            &user_token,
                            src_address,
                            dest_address,
                            heartbeat_response_success_payload_encryption,
                        )?;
                        let mut agent_message_framed_write = agent_message_framed_write.lock().await;
                        agent_message_framed_write.send(heartbeat_response_success).await?;
                        Ok::<_, anyhow::Error>(())
                    });
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::DomainNameResolve) => {
                    let agent_message_framed_write = agent_message_framed_write.clone();
                    let user_token = user_token.clone();
                    tokio::spawn(async move {
                        let domain_resolve_request: DomainResolveRequestPayload = agent_message_payload_data.try_into()?;
                        let request_id = domain_resolve_request.request_id;
                        let domain_name = domain_resolve_request.domain_name;
                        trace!("Receive agent domain resolve message, request id: {request_id}, domain name: {domain_name}");
                        let resolved_ip_addresses = match dns_lookup::lookup_host(&domain_name) {
                            Ok(v) => v,
                            Err(e) => {
                                error!("Fail to resolve domain name because of error: {e:?}");
                                let domain_resolve_fail_response_encryption =
                                    ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                                let domain_resolve_fail_response = MessageUtil::create_proxy_domain_resolve_fail_response(
                                    user_token,
                                    request_id,
                                    domain_name,
                                    domain_resolve_fail_response_encryption,
                                )?;
                                let mut agent_message_framed_write = agent_message_framed_write.lock().await;
                                agent_message_framed_write.send(domain_resolve_fail_response).await?;
                                return Ok::<_, anyhow::Error>(());
                            },
                        };
                        debug!("Success resolve domain to ip addresses, request id: {request_id}, domain name:{domain_name}, resolved ip addresses: {resolved_ip_addresses:?}");
                        let resolved_ip_addresses = resolved_ip_addresses
                            .into_iter()
                            .filter_map(|v| match v {
                                IpAddr::V4(ipv4_addr) => Some(ipv4_addr.octets()),
                                IpAddr::V6(_) => None,
                            })
                            .collect::<Vec<[u8; 4]>>();
                        let domain_resolve_success_response_encryption =
                            ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                        let domain_resolve_success_response = MessageUtil::create_proxy_domain_resolve_success_response(
                            user_token,
                            request_id,
                            domain_name,
                            resolved_ip_addresses,
                            domain_resolve_success_response_encryption,
                        )?;
                        let mut agent_message_framed_write = agent_message_framed_write.lock().await;
                        agent_message_framed_write.send(domain_resolve_success_response).await?;
                        Ok::<_, anyhow::Error>(())
                    });
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpInitialize) => {
                    let agent_message_framed_write = agent_message_framed_write.clone();
                    let tcp_initialize_request: TcpInitializeRequestPayload = agent_message_payload_data.try_into()?;
                    let src_address = tcp_initialize_request.src_address;
                    let dest_address = tcp_initialize_request.dest_address;
                    let tcp_session = TcpSession::new(
                        agent_message_framed_write.clone(),
                        user_token.clone(),
                        src_address.clone(),
                        dest_address.clone(),
                    )
                    .await?;
                    self.tcp_session_container
                        .insert(TcpSession::generate_key(&src_address, &dest_address), tcp_session);
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpRelay) => {
                    let tcp_relay_payload: TcpRelayPayload = agent_message_payload_data.try_into()?;
                    let src_address = tcp_relay_payload.src_address;
                    let dest_address = tcp_relay_payload.dest_address;
                    let tcp_session_key = TcpSession::generate_key(&src_address, &dest_address);
                    let data = tcp_relay_payload.data;
                    let Some(tcp_session) = self.tcp_session_container.get_mut(&tcp_session_key) else {
                        return Err(anyhow::anyhow!(format!( "Tcp session not exist for {tcp_session_key}")));
                    };
                    tcp_session.forward(data.as_slice()).await?;
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpDestroy) => {
                    let tcp_destroy_request: TcpDestroyRequestPayload = agent_message_payload_data.try_into()?;
                    let src_address = tcp_destroy_request.src_address;
                    let dest_address = tcp_destroy_request.dest_address;
                    let tcp_session_key = TcpSession::generate_key(&src_address, &dest_address);
                    if let None = self.tcp_session_container.remove(&tcp_session_key) {
                        return Err(anyhow::anyhow!(format!("Tcp session not exist for {tcp_session_key}")));
                    };
                    debug!("Tcp session [{tcp_session_key}] destroyed.")
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpInitialize) => {
                    todo!();
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpRelay) => {
                    todo!();
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpDestory) => {
                    todo!();
                },
            };
        }
        todo!()
    }
}
