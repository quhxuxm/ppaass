use std::{fmt::Debug, sync::Arc};

use std::fmt::Display;
use std::{net::IpAddr, time::Duration};

use anyhow::Result;
use futures::{SinkExt, StreamExt};

use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::timeout,
};

use tracing::{debug, error, info, trace};

use ppaass_common::{
    domain_resolve::DomainResolveRequestPayload, heartbeat::HeartbeatRequestPayload, PpaassMessageGenerator, PpaassMessageParts,
    PpaassMessagePayloadEncryptionSelector,
};
use ppaass_common::{
    generate_uuid, PpaassConnection, PpaassConnectionWrite, PpaassMessageAgentPayload, PpaassMessageAgentPayloadParts, PpaassMessageAgentPayloadType,
    PpaassNetAddress, RsaCryptoFetcher,
};
use ppaass_common::{tcp_loop::TcpLoopInitRequestPayload, PpaassConnectionRead};

use crate::{common::ProxyServerPayloadEncryptionSelector, processor::udp::UdpHandlerBuilder};
use crate::{config::ProxyServerConfig, processor::tcp::TcpHandlerBuilder};

mod tcp;
mod udp;

#[derive(Debug)]
pub(crate) struct AgentConnectionProcessor<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    connection_id: String,
    read_part: PpaassConnectionRead<T, R, String>,
    write_part: PpaassConnectionWrite<T, R, String>,
    agent_address: PpaassNetAddress,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R> AgentConnectionProcessor<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new(agent_io_stream: T, agent_address: PpaassNetAddress, configuration: Arc<ProxyServerConfig>, rsa_crypto_fetcher: Arc<R>) -> Self {
        let connection_id = generate_uuid();
        let ppaass_connection = PpaassConnection::new(
            connection_id.clone(),
            agent_io_stream,
            rsa_crypto_fetcher,
            configuration.get_compress(),
            configuration.get_message_framed_buffer_size(),
        );
        let (read_part, write_part) = ppaass_connection.split();

        Self {
            connection_id,
            read_part,
            write_part,
            agent_address,
            configuration,
        }
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let connection_id = self.connection_id.clone();
        let agent_address = self.agent_address.clone();
        let configuration = self.configuration.clone();
        debug!("Agent connection [{connection_id}] associated with agent address: {agent_address:?}");

        loop {
            let agent_message = self.read_part.next().await;
            let Some(agent_message) = agent_message else {
                error!("Agent connection [{connection_id}] closed in agent side, close the proxy side also.");
                return Ok(());
            };
            let agent_message = match agent_message {
                Ok(v) => v,
                Err(e) => {
                    error!("Agent connection [{connection_id}] get a error when read from agent: {e:?}");
                    return Err(e);
                },
            };
            let PpaassMessageParts { user_token, payload_bytes, .. } = agent_message.split();
            let PpaassMessageAgentPayloadParts {
                payload_type,
                data: agent_message_payload_data,
            } = TryInto::<PpaassMessageAgentPayload>::try_into(payload_bytes)?.split();

            match payload_type {
                PpaassMessageAgentPayloadType::IdleHeartbeat => {
                    if let Err(e) = handle_idle_heartbeat(agent_address.clone(), user_token, agent_message_payload_data, &mut self.write_part).await {
                        error!("Agent connection [{connection_id}] fail to handle idle heartbeat because of error: {e:?}");
                        continue;
                    };
                    continue;
                },
                PpaassMessageAgentPayloadType::DomainNameResolve => {
                    if let Err(e) = handle_domain_name_resolve(user_token, agent_message_payload_data, &mut self.write_part, configuration.clone()).await {
                        error!("Agent connection [{connection_id}] fail to handle domain resolve because of error: {e:?}");
                        continue;
                    };
                    continue;
                },
                PpaassMessageAgentPayloadType::TcpLoopInit => {
                    let tcp_loop_init_request: TcpLoopInitRequestPayload = match agent_message_payload_data.try_into() {
                        Ok(tcp_loop_init_request) => tcp_loop_init_request,
                        Err(e) => {
                            error!("Agent connection [{connection_id}] fail to read tcp loop init request because of error: {e:?}");
                            return Err(e);
                        },
                    };
                    let src_address = tcp_loop_init_request.src_address;
                    let dest_address = tcp_loop_init_request.dest_address;
                    let read_part = self.read_part;
                    let write_part = self.write_part;
                    let tcp_handler_builder = TcpHandlerBuilder::new()
                        .agent_address(agent_address)
                        .agent_connection_id(&connection_id)
                        .agent_connection_write(write_part)
                        .agent_connection_read(read_part)
                        .user_token(user_token)
                        .src_address(src_address)
                        .dest_address(dest_address);
                    let tcp_handler = match tcp_handler_builder.build(configuration).await {
                        Ok(tcp_handler) => tcp_handler,
                        Err(e) => {
                            error!("Agent connection [{connection_id}] fail to build tcp loop because of error: {e:?}");
                            return Err(e);
                        },
                    };
                    let tcp_handler_key = tcp_handler.get_key().to_owned();
                    debug!("Agent connection [{connection_id}] start tcp loop [{tcp_handler_key}]");
                    if let Err(e) = tcp_handler.exec().await {
                        error!("Agent connection [{connection_id}] fail to execute tcp loop because of error: {e:?}");
                        return Err(e);
                    };
                    return Ok(());
                },
                PpaassMessageAgentPayloadType::UdpLoopInit => {
                    info!("Agent connection [{connection_id}] receive udp loop init from agent.");
                    let udp_handler_builder = UdpHandlerBuilder::new()
                        .agent_address(agent_address)
                        .agent_connection_id(&connection_id)
                        .agent_connection_write(self.write_part)
                        .agent_connection_read(self.read_part)
                        .user_token(user_token);
                    let udp_handler = match udp_handler_builder.build().await {
                        Ok(udp_handler) => udp_handler,
                        Err(e) => {
                            error!("Agent connection [{connection_id}] fail to build udp loop because of error: {e:?}");
                            return Err(e);
                        },
                    };
                    let udp_handler_key = udp_handler.get_key().to_owned();
                    debug!("Agent connection [{connection_id}] start udp loop [{udp_handler_key}]");
                    if let Err(e) = udp_handler.exec().await {
                        error!("Agent connection [{connection_id}] fail to execute udp loop because of error: {e:?}");
                        return Err(e);
                    };
                    return Ok(());
                },
            };
        }
    }
}

async fn handle_idle_heartbeat<T, R, I>(
    agent_address: PpaassNetAddress, user_token: String, agent_message_payload_data: Vec<u8>, agent_connection_write: &mut PpaassConnectionWrite<T, R, I>,
) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    let heartbeat_request: HeartbeatRequestPayload = agent_message_payload_data.try_into()?;
    let timestamp_in_request = heartbeat_request.timestamp;
    debug!("Receive agent heartbeat message, agent address: {agent_address}, timestamp: {timestamp_in_request}");
    let heartbeat_response_success_payload_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
    let heartbeat_response_success = PpaassMessageGenerator::generate_heartbeat_response(&user_token, heartbeat_response_success_payload_encryption)?;
    trace!("Send heartbeat response: {heartbeat_response_success:?}");
    agent_connection_write.send(heartbeat_response_success).await?;
    Ok(())
}

async fn handle_domain_name_resolve<T, R, I>(
    user_token: String, agent_message_payload_data: Vec<u8>, agent_connection_write: &mut PpaassConnectionWrite<T, R, I>, configurtion: Arc<ProxyServerConfig>,
) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
    I: AsRef<str> + Send + Sync + Clone + Display + Debug + 'static,
{
    let DomainResolveRequestPayload {
        domain_name,
        request_id,
        src_address,
        dest_address,
    } = agent_message_payload_data.try_into()?;
    let domain_name_clone = domain_name.clone();
    let request_id_clone = request_id.clone();
    let user_token_clone = user_token.clone();
    let src_address_clone = src_address.clone();
    let dest_address_clone = dest_address.clone();

    let resolve_domain_name_result = timeout(Duration::from_secs(configurtion.get_domain_name_resolve_tomeout()), async move {
        let domain_name = domain_name_clone;
        let request_id = request_id_clone;
        let src_address = src_address_clone;
        let dest_address = dest_address_clone;
        let user_token = user_token_clone;
        let resolved_ip_addresses = match dns_lookup::lookup_host(&domain_name) {
            Ok(resolved_ip_addresses) => resolved_ip_addresses,
            Err(e) => {
                error!("Fail to resolve domain name because of error: {e:?}");
                let domain_resolve_fail_response_encryption = ProxyServerPayloadEncryptionSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let domain_resolve_fail_response = PpaassMessageGenerator::generate_domain_resolve_fail_response(
                    user_token,
                    request_id,
                    domain_name,
                    src_address,
                    dest_address,
                    domain_resolve_fail_response_encryption,
                )?;
                if let Err(e) = agent_connection_write.send(domain_resolve_fail_response).await {
                    error!("Fail to write resolve domain name faile response to agent because of error: {e:?}");
                };
                return Err(anyhow::anyhow!(e));
            },
        };
        Ok((resolved_ip_addresses, agent_connection_write))
    })
    .await;
    match resolve_domain_name_result {
        Err(_e) => {
            error!("Fail to resolve domain name because of timeout, domain name: {domain_name}, source address: {src_address}, destination address: {dest_address}.");
            Err(anyhow::anyhow!(format!("Fail to resolve domain name because of timeout, domain name: {domain_name}, source address: {src_address}, destination address: {dest_address}.")))
        },
        Ok(Err(e)) => Err(e),
        Ok(Ok((resolved_ip_addresses, agent_connection_write))) => {
            debug!(
                "Success resolve domain to ip addresses, request id: {request_id}, domain name:{domain_name}, resolved ip addresses: {resolved_ip_addresses:?}"
            );
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
                src_address,
                dest_address,
                domain_resolve_success_response_encryption,
            )?;
            if let Err(e) = agent_connection_write.send(domain_resolve_success_response).await {
                error!("Fail to write resolve domain name success response to agent because of error: {e:?}");
                return Err(e);
            };
            Ok(())
        },
    }
}
