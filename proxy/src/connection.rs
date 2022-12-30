use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{net::IpAddr, time::Duration};

use anyhow::Result;
use futures::{Sink, SinkExt, Stream, StreamExt};
use pin_project::{pin_project, pinned_drop};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::timeout,
};
use tokio_util::codec::Framed;
use tracing::{debug, error, trace};

use ppaass_common::tcp_loop::TcpLoopInitRequestPayload;
use ppaass_common::{
    codec::PpaassMessageCodec, generate_uuid, PpaassMessage, PpaassMessageAgentPayload, PpaassMessageAgentPayloadParts, PpaassMessageAgentPayloadType,
    PpaassNetAddress, RsaCryptoFetcher,
};
use ppaass_common::{
    domain_resolve::DomainResolveRequestPayload, heartbeat::HeartbeatRequestPayload, PpaassMessageGenerator, PpaassMessageParts,
    PpaassMessagePayloadEncryptionSelector,
};

use crate::common::ProxyServerPayloadEncryptionSelector;
use crate::types::{AgentMessageFramedRead, AgentMessageFramedWrite};
use crate::{config::ProxyServerConfig, connection::tcp_loop::TcpLoopBuilder};

mod tcp_loop;
mod udp_loop;

#[derive(Debug)]
pub(crate) struct AgentConnection<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    id: String,
    read: AgentConnectionRead<T, R>,
    write: AgentConnectionWrite<T, R>,
    agent_address: PpaassNetAddress,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R> AgentConnection<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new(agent_io: T, agent_address: PpaassNetAddress, configuration: Arc<ProxyServerConfig>, rsa_crypto_fetcher: Arc<R>) -> Self {
        let agent_message_codec = PpaassMessageCodec::new(configuration.get_compress(), rsa_crypto_fetcher);
        let agent_message_framed = Framed::with_capacity(agent_io, agent_message_codec, configuration.get_message_framed_buffer_size());
        let (agent_message_framed_write, agent_message_framed_read) = agent_message_framed.split();
        let id = generate_uuid();
        let read = AgentConnectionRead::new(id.clone(), configuration.clone(), agent_address.clone(), agent_message_framed_read);
        let write = AgentConnectionWrite::new(id.clone(), configuration.clone(), agent_address.clone(), agent_message_framed_write);
        Self {
            id,
            read,
            write,
            agent_address,
            configuration,
        }
    }

    pub(crate) async fn exec(mut self) -> Result<()> {
        let connection_id = self.id.clone();
        let agent_address = self.agent_address.clone();
        let configuration = self.configuration.clone();
        debug!("Agent connection [{connection_id}] associated with agent address: {agent_address:?}");

        loop {
            let agent_message = self.read.next().await;
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
                    if let Err(e) = handle_idle_heartbeat(agent_address.clone(), user_token, agent_message_payload_data, &mut self.write).await {
                        error!("Agent connection [{connection_id}] fail to handle idle heartbeat because of error: {e:?}");
                        continue;
                    };
                    continue;
                },
                PpaassMessageAgentPayloadType::DomainNameResolve => {
                    if let Err(e) = handle_domain_name_resolve(user_token, agent_message_payload_data, &mut self.write, configuration.clone()).await {
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
                    let read = self.read;
                    let write = self.write;
                    let tcp_loop_builder = TcpLoopBuilder::new()
                        .agent_address(agent_address)
                        .agent_connection_id(&connection_id)
                        .agent_connection_write(write)
                        .agent_connection_read(read)
                        .user_token(user_token)
                        .src_address(src_address)
                        .dest_address(dest_address);
                    let tcp_loop = match tcp_loop_builder.build(configuration).await {
                        Ok(tcp_loop) => tcp_loop,
                        Err(e) => {
                            error!("Agent connection [{connection_id}] fail to build tcp loop because of error: {e:?}");
                            return Err(e);
                        },
                    };
                    let tcp_loop_key = tcp_loop.get_key().to_owned();
                    debug!("Agent connection [{connection_id}] start tcp loop [{tcp_loop_key}]");
                    if let Err(e) = tcp_loop.exec().await {
                        error!("Agent connection [{connection_id}] fail to execute tcp loop because of error: {e:?}");
                        return Err(e);
                    };
                    return Ok(());
                },
                PpaassMessageAgentPayloadType::UdpLoopInit => todo!(),
            };
        }
    }
}

#[pin_project(PinnedDrop)]
#[derive(Debug)]
pub(crate) struct AgentConnectionWrite<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    connection_id: String,
    configuration: Arc<ProxyServerConfig>,
    agent_address: PpaassNetAddress,
    #[pin]
    agent_message_framed_write: Option<AgentMessageFramedWrite<T, R>>,
}

#[pinned_drop]
impl<T, R> PinnedDrop for AgentConnectionWrite<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    fn drop(self: Pin<&mut Self>) {
        let mut this = self.project();
        let connection_id = this.connection_id.clone();
        if let Some(mut agent_message_framed_write) = this.agent_message_framed_write.take() {
            tokio::spawn(async move {
                if let Err(e) = agent_message_framed_write.close().await {
                    error!("Fail to close agent connection because of error: {e:?}");
                };
                debug!("Agent connection writer [{connection_id}] dropped")
            });
        }
    }
}

impl<T, R> AgentConnectionWrite<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new(
        connection_id: String, configuration: Arc<ProxyServerConfig>, agent_address: PpaassNetAddress,
        agent_message_framed_write: AgentMessageFramedWrite<T, R>,
    ) -> Self {
        Self {
            connection_id,
            configuration,
            agent_address,
            agent_message_framed_write: Some(agent_message_framed_write),
        }
    }
}

impl<T, R> Sink<PpaassMessage> for AgentConnectionWrite<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    type Error = anyhow::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let agent_message_framed_write = this.agent_message_framed_write.as_pin_mut();
        if let Some(agent_message_framed_write) = agent_message_framed_write {
            agent_message_framed_write.poll_ready(cx)
        } else {
            Poll::Ready(Err(anyhow::anyhow!("Agent message framed not exist.")))
        }
    }

    fn start_send(self: Pin<&mut Self>, item: PpaassMessage) -> Result<(), Self::Error> {
        let this = self.project();
        let agent_message_framed_write = this.agent_message_framed_write.as_pin_mut();
        if let Some(agent_message_framed_write) = agent_message_framed_write {
            agent_message_framed_write.start_send(item)
        } else {
            Err(anyhow::anyhow!("Agent message framed not exist."))
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let agent_message_framed_write = this.agent_message_framed_write.as_pin_mut();
        if let Some(agent_message_framed_write) = agent_message_framed_write {
            agent_message_framed_write.poll_flush(cx)
        } else {
            Poll::Ready(Err(anyhow::anyhow!("Agent message framed not exist.")))
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        let agent_message_framed_write = this.agent_message_framed_write.as_pin_mut();
        if let Some(agent_message_framed_write) = agent_message_framed_write {
            agent_message_framed_write.poll_close(cx)
        } else {
            Poll::Ready(Err(anyhow::anyhow!("Agent message framed not exist.")))
        }
    }
}

#[pin_project]
#[derive(Debug)]
pub(crate) struct AgentConnectionRead<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    connection_id: String,
    configuration: Arc<ProxyServerConfig>,
    agent_address: PpaassNetAddress,
    #[pin]
    agent_message_framed_read: AgentMessageFramedRead<T, R>,
}

impl<T, R> AgentConnectionRead<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new(
        connection_id: String, configuration: Arc<ProxyServerConfig>, agent_address: PpaassNetAddress, agent_message_framed_read: AgentMessageFramedRead<T, R>,
    ) -> Self {
        Self {
            connection_id,
            configuration,
            agent_message_framed_read,
            agent_address,
        }
    }
}

impl<T, R> Stream for AgentConnectionRead<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    type Item = Result<PpaassMessage>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        this.agent_message_framed_read.poll_next(cx)
    }
}

async fn handle_idle_heartbeat<T, R>(
    agent_address: PpaassNetAddress, user_token: String, agent_message_payload_data: Vec<u8>, agent_connection_write: &mut AgentConnectionWrite<T, R>,
) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
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

async fn handle_domain_name_resolve<T, R>(
    user_token: String, agent_message_payload_data: Vec<u8>, agent_connection_write: &mut AgentConnectionWrite<T, R>, configurtion: Arc<ProxyServerConfig>,
) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    let DomainResolveRequestPayload {
        domain_name,
        request_id,
        src_address,
        dest_address,
    } = agent_message_payload_data.try_into()?;
    let domain_name_for_error = domain_name.clone();
    let src_address_for_error = src_address.clone();
    let dest_address_for_error = dest_address.clone();
    if let Err(_e) = timeout(Duration::from_secs(configurtion.get_domain_name_resolve_tomeout()), async move {
        trace!("Receive agent domain resolve message, request id: {request_id}, domain name: {domain_name}");
        let resolved_ip_addresses = match dns_lookup::lookup_host(&domain_name) {
            Ok(v) => v,
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
                agent_connection_write.send(domain_resolve_fail_response).await?;
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
            src_address,
            dest_address,
            domain_resolve_success_response_encryption,
        )?;
        agent_connection_write.send(domain_resolve_success_response).await?;
        Ok(())
    })
    .await
    {
        error!("Fail to resolve domain name because of timeout, domain name: {domain_name_for_error}, source address: {src_address_for_error}, destination address: {dest_address_for_error}.");
        return Err(anyhow::anyhow!(format!("Fail to resolve domain name because of timeout, domain name: {domain_name_for_error}, source address: {src_address_for_error}, destination address: {dest_address_for_error}.")));
    };
    Ok(())
}
