use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use futures::StreamExt;

use tokio::io::{AsyncRead, AsyncWrite};

use tracing::{debug, error, info};

use ppaass_common::{dns::DnsLookupRequest, tcp::TcpInitRequest, udp::UdpData, CommonError};
use ppaass_common::{
    PpaassConnection, PpaassMessageAgentPayload, PpaassMessageAgentPayloadParts, PpaassMessageAgentPayloadType, PpaassNetAddress, RsaCryptoFetcher,
};
use ppaass_common::{PpaassConnectionParts, PpaassMessageParts};

use crate::{
    config::ProxyServerConfig,
    error::ProxyError,
    processor::{dns::DnsLookupHandler, udp::UdpHandler},
};

use self::tcp::TcpHandler;

mod dns;
mod tcp;
mod udp;

#[derive(Debug)]
pub(crate) struct AgentConnectionProcessor<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    agent_connection: PpaassConnection<T, R, String>,
    agent_address: PpaassNetAddress,
    configuration: Arc<ProxyServerConfig>,
}

impl<T, R> AgentConnectionProcessor<T, R>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    R: RsaCryptoFetcher + Send + Sync + 'static,
{
    pub(crate) fn new(agent_tcp_stream: T, agent_address: PpaassNetAddress, configuration: Arc<ProxyServerConfig>, rsa_crypto_fetcher: Arc<R>) -> Self {
        let agent_connection = PpaassConnection::new(
            agent_address.to_string(),
            agent_tcp_stream,
            rsa_crypto_fetcher,
            configuration.get_compress(),
            configuration.get_message_framed_buffer_size(),
        );
        Self {
            agent_connection,
            agent_address,
            configuration,
        }
    }

    pub(crate) async fn exec(self) -> Result<(), ProxyError> {
        let agent_address = self.agent_address;
        let configuration = self.configuration;
        let PpaassConnectionParts {
            read: mut agent_connection_read,
            write: agent_connection_write,
            id: connection_id,
        } = self.agent_connection.split();

        let agent_message = match agent_connection_read.next().await {
            Some(agent_message) => agent_message?,
            None => {
                error!("Agent connection {connection_id} closed in agent side, close the proxy side also.");
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
            data: agent_message_payload,
        } = TryInto::<PpaassMessageAgentPayload>::try_into(agent_message_payload)?.split();

        match payload_type {
            PpaassMessageAgentPayloadType::TcpInit => {
                let tcp_init_request: TcpInitRequest = agent_message_payload.try_into()?;
                let src_address = tcp_init_request.src_address;
                let dst_address = tcp_init_request.dst_address;
                let tcp_handler = TcpHandler::new(
                    connection_id,
                    user_token,
                    agent_connection_read,
                    agent_connection_write,
                    agent_address,
                    src_address,
                    dst_address,
                    configuration,
                );
                tcp_handler.exec().await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::UdpData => {
                info!("Agent connection {connection_id} receive udp data from agent.");
                let udp_data: UdpData = agent_message_payload.try_into()?;
                // let udp_handler_builder = UdpHandlerBuilder::new()
                //     .agent_address(agent_address)
                //     .ppaass_connection_id(connection_id.clone())
                //     .ppaass_connection_write(agent_connection_write)
                //     .ppaass_connection_read(agent_connection_read)
                //     .user_token(user_token);
                // let udp_handler = match udp_handler_builder.build(configuration.clone()).await {
                //     Ok(udp_handler) => udp_handler,
                //     Err(e) => {
                //         error!("Agent connection {connection_id} fail to build udp handler because of error: {e:?}");
                //         return Err(e);
                //     },
                // };
                let udp_handler = UdpHandler::new(connection_id, user_token, agent_address, agent_connection_write, configuration);
                udp_handler.exec(udp_data).await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::DnsLookupRequest => {
                info!("Agent connection {connection_id} receive dns lookup request from agent.");
                let dns_lookup_request: DnsLookupRequest = agent_message_payload.try_into()?;
                let dns_lookup_handler = DnsLookupHandler::new(connection_id.clone(), agent_address.clone(), user_token, agent_connection_write);
                dns_lookup_handler.exec(dns_lookup_request).await?;
                Ok(())
            },
        }
    }
}
