use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use futures::StreamExt;

use tokio::io::{AsyncRead, AsyncWrite};

use tracing::{debug, error, info};

use ppaass_common::PpaassMessageParts;
use ppaass_common::{
    generate_uuid, PpaassConnection, PpaassConnectionWrite, PpaassMessageAgentPayload, PpaassMessageAgentPayloadParts, PpaassMessageAgentPayloadType,
    PpaassNetAddress, RsaCryptoFetcher,
};
use ppaass_common::{tcp::TcpInitRequest, PpaassConnectionRead};

use crate::{config::ProxyServerConfig, processor::tcp::TcpHandlerBuilder, processor::udp::UdpHandlerBuilder};

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
        let PpaassMessageParts { user_token, payload, .. } = agent_message.split();
        let PpaassMessageAgentPayloadParts {
            payload_type,
            data: agent_message_payload_data,
        } = TryInto::<PpaassMessageAgentPayload>::try_into(payload)?.split();

        match payload_type {
            PpaassMessageAgentPayloadType::TcpInit => {
                let tcp_init_request: TcpInitRequest = match agent_message_payload_data.try_into() {
                    Ok(tcp_init_request) => tcp_init_request,
                    Err(e) => {
                        error!("Agent connection [{connection_id}] fail to read tcp loop init request because of error: {e:?}");
                        return Err(e);
                    },
                };
                let src_address = tcp_init_request.src_address;
                let dst_address = tcp_init_request.dst_address;
                let read_part = self.read_part;
                let write_part = self.write_part;
                let tcp_handler_builder = TcpHandlerBuilder::new()
                    .agent_address(agent_address)
                    .agent_connection_id(&connection_id)
                    .agent_connection_write(write_part)
                    .agent_connection_read(read_part)
                    .user_token(user_token)
                    .src_address(src_address)
                    .dst_address(dst_address);
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
                Ok(())
            },
            PpaassMessageAgentPayloadType::UdpInit => {
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
                Ok(())
            },
        }
    }
}
