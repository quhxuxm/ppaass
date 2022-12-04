use std::net::IpAddr;
use std::sync::Arc;

use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use ppaass_common::{
    generate_uuid, PpaassMessage, PpaassMessageAgentPayload, PpaassMessageAgentPayloadParts, PpaassMessageAgentPayloadType, PpaassMessageFramed,
    PpaassNetAddress, RsaCryptoFetcher,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    task::JoinHandle,
};

use crate::config::ProxyServerConfig;
use crate::{common::ProxyServerPayloadEncryptionSelector, connection::tcp_loop::TcpLoop};
use anyhow::Result;
use ppaass_common::tcp_loop::TcpLoopInitRequestPayload;
use ppaass_common::{
    domain_resolve::DomainResolveRequestPayload, heartbeat::HeartbeatRequestPayload, PpaassMessageGenerator, PpaassMessageParts,
    PpaassMessagePayloadEncryptionSelector,
};

use tracing::{debug, error, info, trace};

mod tcp_loop;
mod udp_loop;

type AgentMessageFramedRead<T, R> = SplitStream<PpaassMessageFramed<T, R>>;
type AgentMessageFramedWrite<T, R> = SplitSink<PpaassMessageFramed<T, R>, PpaassMessage>;

#[derive(Debug)]
pub(crate) struct AgentConnection<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    id: String,
    agent_message_framed: PpaassMessageFramed<T, R>,
    agent_address: PpaassNetAddress,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R> AgentConnection<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new(agent_io: T, agent_address: PpaassNetAddress, configuration: Arc<ProxyServerConfig>, rsa_fetcher: Arc<R>) -> Self {
        let message_framed = PpaassMessageFramed::new(agent_io, configuration.get_compress(), 1024 * 64, rsa_fetcher);
        Self {
            id: generate_uuid(),
            agent_message_framed: message_framed,
            agent_address,
            configuration,
        }
    }

    pub(crate) async fn exec(self) -> Result<()> {
        let connection_id = self.id;
        let agent_address = self.agent_address;
        debug!("Agent connection [{connection_id}] associated with agent address: {agent_address:?}");
        let agent_message_framed = self.agent_message_framed;
        let (mut agent_message_framed_write, mut agent_message_framed_read) = agent_message_framed.split();
        loop {
            let agent_message = agent_message_framed_read.next().await;
            let Some(agent_message) = agent_message else {
                error!("Agent connection [{connection_id}] closed in agent side, close the proxy side also.");
                return Ok(());
            };
            let agent_message = match agent_message {
                Ok(v) => v,
                Err(e) => {
                    error!("Agent connection [{connection_id}] get a error when read from agent: {e:?}");
                    return Err(anyhow::anyhow!(e));
                },
            };
            let PpaassMessageParts {
                id: agent_message_id,
                user_token,
                payload_bytes,
                ..
            } = agent_message.split();
            let PpaassMessageAgentPayloadParts {
                payload_type,
                data: agent_message_payload_data,
            } = TryInto::<PpaassMessageAgentPayload>::try_into(payload_bytes)?.split();

            match payload_type {
                PpaassMessageAgentPayloadType::IdleHeartbeat => {
                    if let Err(e) = handle_idle_heartbeat(agent_address.clone(), user_token, agent_message_payload_data, &mut agent_message_framed_write).await
                    {
                        error!("Agent connection [{connection_id}] fail to handle idle heartbeat because of error: {e:?}");
                        continue;
                    };
                    continue;
                },
                PpaassMessageAgentPayloadType::DomainNameResolve => {
                    if let Err(e) = handle_domain_name_resolve(user_token, agent_message_payload_data, &mut agent_message_framed_write).await {
                        error!("Agent connection [{connection_id}] fail to handle domain resolve because of error: {e:?}");
                        continue;
                    };
                    continue;
                },
                PpaassMessageAgentPayloadType::TcpLoopInit => {
                    let tcp_loop_init_request: TcpLoopInitRequestPayload = agent_message_payload_data.try_into()?;
                    let src_address = tcp_loop_init_request.src_address;
                    let dest_address = tcp_loop_init_request.dest_address;
                    let tcp_loop = TcpLoop::new(
                        agent_message_id,
                        agent_message_framed_read,
                        agent_message_framed_write,
                        user_token.clone(),
                        agent_address.clone(),
                        src_address.clone(),
                        dest_address.clone(),
                    )
                    .await?;
                    let tcp_loop_key = tcp_loop.get_key().to_owned();
                    debug!("Agent connection [{connection_id}] start tcp loop [{tcp_loop_key}]");
                    tcp_loop.start().await?;
                    break;
                },
                PpaassMessageAgentPayloadType::UdpLoopInit => todo!(),
            };
        }

        Ok(())
    }
}

async fn handle_idle_heartbeat<T, R>(
    agent_address: PpaassNetAddress, user_token: String, agent_message_payload_data: Vec<u8>,
    message_framed_write: &mut SplitSink<PpaassMessageFramed<T, R>, PpaassMessage>,
) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
    R: RsaCryptoFetcher,
{
    let heartbeat_request: HeartbeatRequestPayload = agent_message_payload_data.try_into()?;
    let timestamp_in_request = heartbeat_request.timestamp;
    trace!("Receive agent heartbeat message, agent address: {agent_address}, timestamp: {timestamp_in_request}");
    let heartbeat_response_success_payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
    let heartbeat_response_success = PpaassMessageGenerator::generate_heartbeat_response(&user_token, heartbeat_response_success_payload_encryption)?;
    info!("Send heartbeat response: {heartbeat_response_success:?}");
    message_framed_write.send(heartbeat_response_success).await?;
    Ok(())
}

async fn handle_domain_name_resolve<T, R>(
    user_token: String, agent_message_payload_data: Vec<u8>, message_framed_write: &mut SplitSink<PpaassMessageFramed<T, R>, PpaassMessage>,
) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
    R: RsaCryptoFetcher,
{
    let domain_resolve_request: DomainResolveRequestPayload = agent_message_payload_data.try_into()?;
    let request_id = domain_resolve_request.request_id;
    let domain_name = domain_resolve_request.domain_name;
    trace!("Receive agent domain resolve message, request id: {request_id}, domain name: {domain_name}");
    let resolved_ip_addresses = match dns_lookup::lookup_host(&domain_name) {
        Ok(v) => v,
        Err(e) => {
            error!("Fail to resolve domain name because of error: {e:?}");
            let domain_resolve_fail_response_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
            let domain_resolve_fail_response =
                PpaassMessageGenerator::generate_domain_resolve_fail_response(user_token, request_id, domain_name, domain_resolve_fail_response_encryption)?;
            message_framed_write.send(domain_resolve_fail_response).await?;
            return Err(anyhow::anyhow!(e));
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
    let domain_resolve_success_response_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
    let domain_resolve_success_response = PpaassMessageGenerator::generate_domain_resolve_success_response(
        user_token,
        request_id,
        domain_name,
        resolved_ip_addresses,
        domain_resolve_success_response_encryption,
    )?;
    message_framed_write.send(domain_resolve_success_response).await?;
    Ok(())
}
