use std::{collections::HashMap, time::Duration};

use std::net::IpAddr;
use std::{net::SocketAddr, sync::Arc};

use futures::{SinkExt, StreamExt};
use ppaass_common::generate_uuid;
use tokio::{
    select,
    sync::{mpsc::channel, Mutex},
};

use crate::common::ProxyServerPayloadEncryptionSelector;
use crate::tunnel::tcp_session::TcpSession;
use crate::{common::AgentMessageFramed, config::ProxyServerConfig};
use anyhow::Result;
use ppaass_protocol::tcp_destroy::TcpDestroyRequestPayload;
use ppaass_protocol::tcp_initialize::TcpInitializeRequestPayload;
use ppaass_protocol::tcp_relay::TcpRelayPayload;
use ppaass_protocol::{
    domain_resolve::DomainResolveRequestPayload, heartbeat::HeartbeatRequestPayload, PpaassMessageAgentPayloadTypeValue, PpaassMessageParts,
    PpaassMessagePayload, PpaassMessagePayloadEncryptionSelector, PpaassMessagePayloadParts, PpaassMessagePayloadType, PpaassMessageUtil,
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
    last_heartbeat_timestamp: i64,
}

impl TcpTunnel {
    pub(crate) fn new(agent_message_framed: AgentMessageFramed, agent_socket_address: SocketAddr, configuration: Arc<ProxyServerConfig>) -> Self {
        Self {
            id: generate_uuid(),
            agent_message_framed,
            agent_socket_address,
            configuration,
            tcp_session_container: HashMap::new(),
            last_heartbeat_timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let tunnel_id = self.id;
        let agent_socket_address = self.agent_socket_address;
        debug!("tunnel [{tunnel_id}] begin for agent connection: {agent_socket_address}");
        let agent_message_framed = self.agent_message_framed;
        let (agent_message_framed_write, mut agent_message_framed_read) = agent_message_framed.split();
        let agent_message_framed_write = Arc::new(Mutex::new(agent_message_framed_write));
        let last_heartbeat_timestamp = Arc::new(Mutex::new(self.last_heartbeat_timestamp));
        let last_heartbeat_timestamp_for_checker = last_heartbeat_timestamp.clone();
        let (heartbeat_timeout_sender, mut heartbeat_timeout_receiver) = channel::<bool>(1);
        let tunnel_id_for_heartbeat = tunnel_id.clone();

        tokio::spawn(async move {
            debug!("Start heartbeat task for tunnel [{tunnel_id_for_heartbeat}]");
            let mut check_interval = tokio::time::interval(Duration::from_secs(10));
            let current_last_heartbeat_timestamp = last_heartbeat_timestamp_for_checker.lock().await;
            loop {
                check_interval.tick().await;
                let deta = chrono::Utc::now().timestamp_millis() - *current_last_heartbeat_timestamp;
                if deta <= 1000 * 120 {
                    continue;
                }
                error!("Tunnel {tunnel_id_for_heartbeat} idle timeout.");
                match heartbeat_timeout_sender.send(true).await {
                    Ok(()) => break,
                    Err(e) => {
                        error!("Tunnel {tunnel_id_for_heartbeat} idle timeout because fail to notify because of error: {e:?}");
                        break;
                    },
                };
            }
        });

        while let Some(agent_message) = select! {
            _ = heartbeat_timeout_receiver.recv()=>{
                return Err(anyhow::anyhow!("Tunnel [{tunnel_id}] idle timeout."));
            },
            v = agent_message_framed_read.next()=>v
        } {
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
                    let last_heartbeat_timestamp = last_heartbeat_timestamp.clone();
                    tokio::spawn(async move {
                        let heartbeat_request: HeartbeatRequestPayload = agent_message_payload_data.try_into()?;
                        let timestamp_in_request = heartbeat_request.timestamp;
                        trace!("Receive agent heartbeat message, agent address: {agent_socket_address}, timestamp: {timestamp_in_request}");
                        let mut last_heartbeat_timestamp = last_heartbeat_timestamp.lock().await;
                        *last_heartbeat_timestamp = chrono::Utc::now().timestamp_millis();
                        let heartbeat_response_success_payload_encryption =
                            ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                        let heartbeat_response_success =
                            PpaassMessageUtil::create_proxy_heartbeat_response(&user_token, heartbeat_response_success_payload_encryption)?;
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
                                let domain_resolve_fail_response = PpaassMessageUtil::create_proxy_domain_resolve_fail_response(
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
                        let domain_resolve_success_response = PpaassMessageUtil::create_proxy_domain_resolve_success_response(
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
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionInitialize) => {
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
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionRelay) => {
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
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::TcpSessionDestroy) => {
                    let tcp_destroy_request: TcpDestroyRequestPayload = agent_message_payload_data.try_into()?;
                    let src_address = tcp_destroy_request.src_address;
                    let dest_address = tcp_destroy_request.dest_address;
                    let tcp_session_key = TcpSession::generate_key(&src_address, &dest_address);
                    if let None = self.tcp_session_container.remove(&tcp_session_key) {
                        return Err(anyhow::anyhow!(format!("Tcp session not exist for {tcp_session_key}")));
                    };
                    debug!("Tcp session [{tcp_session_key}] destroyed.")
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpSessionInitialize) => {
                    todo!();
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpSessionRelay) => {
                    todo!();
                },
                PpaassMessagePayloadType::AgentPayload(PpaassMessageAgentPayloadTypeValue::UdpSessionDestroy) => {
                    todo!();
                },
            };
        }
        Ok(())
    }
}
