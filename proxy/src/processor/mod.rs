use std::{fmt::Debug, sync::Arc};

use anyhow::Result;
use futures::StreamExt;

use tokio::io::{AsyncRead, AsyncWrite};

use tracing::{error, info};

use ppaass_common::{dns::DnsLookupRequest, tcp::TcpInitRequest, udp::UdpData};
use ppaass_common::{PpaassConnection, PpaassMessageAgentPayload, PpaassMessageAgentPayloadType, PpaassNetAddress, RsaCryptoFetcher};
use ppaass_common::{PpaassConnectionParts, PpaassMessage};

use crate::{
    config::ProxyServerConfig,
    error::ProxyError,
    processor::{
        dns::{DnsLookupHandler, DnsLookupHandlerKey},
        udp::{UdpHandler, UdpHandlerKey},
    },
};

use self::tcp::{TcpHandler, TcpHandlerKey};

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
            configuration.get_agent_recive_buffer_size(),
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
            read_part: mut agent_connection_read,
            write_part: agent_connection_write,
            id: connection_id,
        } = self.agent_connection.split();

        let agent_message = match agent_connection_read.next().await {
            Some(agent_message) => agent_message?,
            None => {
                error!("Agent connection {connection_id} closed in agent side, close the proxy side also.");
                return Ok(());
            },
        };
        let PpaassMessage { user_token, payload, .. } = agent_message;
        let PpaassMessageAgentPayload { payload_type, data } = payload.as_slice().try_into()?;
        match payload_type {
            PpaassMessageAgentPayloadType::TcpInit => {
                let tcp_init_request: TcpInitRequest = data.as_slice().try_into()?;
                let src_address = tcp_init_request.src_address;
                let dst_address = tcp_init_request.dst_address;
                let tcp_handler_key = TcpHandlerKey::new(connection_id, user_token, agent_address, src_address, dst_address);
                let tcp_handler = TcpHandler::new(tcp_handler_key, agent_connection_read, agent_connection_write, configuration);
                tcp_handler.exec().await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::UdpData => {
                info!("Agent connection {connection_id} receive udp data from agent.");
                let UdpData {
                    src_address,
                    dst_address,
                    data: udp_raw_data,
                    ..
                } = data.as_slice().try_into()?;
                let udp_handler_key = UdpHandlerKey::new(connection_id, user_token, agent_address, src_address, dst_address);
                let udp_handler = UdpHandler::new(udp_handler_key, agent_connection_write, configuration);
                udp_handler.exec(udp_raw_data).await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::DnsLookupRequest => {
                info!("Agent connection {connection_id} receive dns lookup request from agent.");
                let dns_lookup_request: DnsLookupRequest = data.as_slice().try_into()?;
                let dns_lookup_handler_key = DnsLookupHandlerKey::new(connection_id, user_token, agent_address);
                let dns_lookup_handler = DnsLookupHandler::new(dns_lookup_handler_key, agent_connection_write);
                dns_lookup_handler.exec(dns_lookup_request).await?;
                Ok(())
            },
        }
    }
}