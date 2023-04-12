use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use futures::StreamExt;

use tokio::io::{AsyncRead, AsyncWrite};

use tracing::{debug, error, info};

use ppaass_common::tcp::TcpInitRequest;
use ppaass_common::{
    PpaassConnection, PpaassMessageAgentPayload, PpaassMessageAgentPayloadParts, PpaassMessageAgentPayloadType, PpaassNetAddress, RsaCryptoFetcher,
};
use ppaass_common::{PpaassConnectionParts, PpaassMessageParts};

use crate::{config::ProxyServerConfig, processor::tcp::TcpHandlerBuilder, processor::udp::UdpHandlerBuilder};

mod tcp;
mod udp;

#[derive(Debug)]
pub(crate) struct AgentConnectionProcessor<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    ppaass_connection: PpaassConnection<T, R, String>,
    agent_address: PpaassNetAddress,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R> AgentConnectionProcessor<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new(agent_io_stream: T, agent_address: PpaassNetAddress, configuration: Arc<ProxyServerConfig>, rsa_crypto_fetcher: Arc<R>) -> Self {
        let ppaass_connection = PpaassConnection::new(
            agent_address.to_string(),
            agent_io_stream,
            rsa_crypto_fetcher,
            configuration.get_compress(),
            configuration.get_message_framed_buffer_size(),
        );
        Self {
            ppaass_connection,
            agent_address,
            configuration,
        }
    }

    pub(crate) async fn exec(self) -> Result<()> {
        let agent_address = self.agent_address.clone();
        let configuration = self.configuration.clone();
        let PpaassConnectionParts {
            read: mut ppaass_connection_read,
            write: ppaass_connection_write,
            id: ppaass_connection_id,
        } = self.ppaass_connection.split();
        debug!("Agent connection [{ppaass_connection_id}] associated with agent address: {agent_address:?}");

        let agent_message = match ppaass_connection_read.next().await {
            Some(Ok(agent_message)) => agent_message,
            Some(Err(e)) => {
                error!("Agent connection [{ppaass_connection_id}] get a error when read from agent: {e:?}");
                return Err(e);
            },
            None => {
                error!("Agent connection [{ppaass_connection_id}] closed in agent side, close the proxy side also.");
                return Ok(());
            },
        };
        let PpaassMessageParts {
            user_token,
            payload: agent_message_payload,
            ..
        } = agent_message.split();
        let PpaassMessageAgentPayloadParts {
            payload_type,
            data: agent_message_payload_raw_data,
        } = TryInto::<PpaassMessageAgentPayload>::try_into(agent_message_payload)?.split();

        match payload_type {
            PpaassMessageAgentPayloadType::TcpInit => {
                let tcp_init_request: TcpInitRequest = match agent_message_payload_raw_data.try_into() {
                    Ok(tcp_init_request) => tcp_init_request,
                    Err(e) => {
                        error!("Agent connection [{ppaass_connection_id}] fail to read tcp loop init request because of error: {e:?}");
                        return Err(e);
                    },
                };
                let src_address = tcp_init_request.src_address;
                let dst_address = tcp_init_request.dst_address;
                let tcp_handler_builder = TcpHandlerBuilder::new()
                    .agent_address(agent_address)
                    .src_address(src_address)
                    .dst_address(dst_address)
                    .ppaass_connection_write(ppaass_connection_write)
                    .ppaass_connection_read(ppaass_connection_read)
                    .user_token(user_token)
                    .ppaass_connection_id(ppaass_connection_id.clone());
                let tcp_handler = match tcp_handler_builder.build(configuration).await {
                    Ok(tcp_handler) => tcp_handler,
                    Err(e) => {
                        error!("Agent connection [{ppaass_connection_id}] fail to build tcp handler because of error: {e:?}");
                        return Err(e);
                    },
                };
                tcp_handler.exec().await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::UdpInit => {
                info!("Agent connection [{ppaass_connection_id}] receive udp loop init from agent.");
                let udp_handler_builder = UdpHandlerBuilder::new()
                    .agent_address(agent_address)
                    .ppaass_connection_id(ppaass_connection_id.clone())
                    .ppaass_connection_write(ppaass_connection_write)
                    .ppaass_connection_read(ppaass_connection_read)
                    .user_token(user_token);
                let udp_handler = match udp_handler_builder.build().await {
                    Ok(udp_handler) => udp_handler,
                    Err(e) => {
                        error!("Agent connection [{ppaass_connection_id}] fail to build udp loop because of error: {e:?}");
                        return Err(e);
                    },
                };
                udp_handler.exec().await?;
                Ok(())
            },
        }
    }
}
