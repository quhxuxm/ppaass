use std::fmt::Debug;

use anyhow::Result;
use futures::StreamExt;

use tokio::io::{AsyncRead, AsyncWrite};

use log::{error, info};

use ppaass_common::PpaassMessage;
use ppaass_common::{dns::DnsLookupRequest, tcp::TcpInitRequest, udp::UdpData};
use ppaass_common::{PpaassConnection, PpaassMessageAgentPayload, PpaassMessageAgentPayloadType, PpaassNetAddress};

use crate::{
    config::PROXY_CONFIG,
    crypto::{ProxyServerRsaCryptoFetcher, RSA_CRYPTO},
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
pub(crate) struct AgentConnectionProcessor<'r, T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    'r: 'static,
{
    agent_connection: PpaassConnection<'r, T, ProxyServerRsaCryptoFetcher, String>,
    agent_address: PpaassNetAddress,
}

impl<'r, T> AgentConnectionProcessor<'r, T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    'r: 'static,
{
    pub(crate) fn new(agent_tcp_stream: T, agent_address: PpaassNetAddress) -> AgentConnectionProcessor<'r, T> {
        let agent_connection = PpaassConnection::new(
            agent_address.to_string(),
            agent_tcp_stream,
            &*RSA_CRYPTO,
            PROXY_CONFIG.get_compress(),
            PROXY_CONFIG.get_agent_recive_buffer_size(),
        );
        Self {
            agent_connection,
            agent_address,
        }
    }

    pub(crate) async fn exec(mut self) -> Result<(), ProxyError> {
        let agent_address = self.agent_address;

        let agent_message = match self.agent_connection.next().await {
            Some(agent_message) => agent_message?,
            None => {
                error!(
                    "Agent connection {} closed in agent side, close the proxy side also.",
                    self.agent_connection.get_connection_id()
                );
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
                let tcp_handler_key = TcpHandlerKey::new(
                    self.agent_connection.get_connection_id().to_string(),
                    user_token,
                    agent_address,
                    src_address,
                    dst_address,
                );
                let tcp_handler = TcpHandler::new(tcp_handler_key, self.agent_connection);
                tcp_handler.exec().await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::UdpData => {
                info!("Agent connection {} receive udp data from agent.", self.agent_connection.get_connection_id());
                let UdpData {
                    src_address,
                    dst_address,
                    data: udp_raw_data,
                    ..
                } = data.as_slice().try_into()?;
                let udp_handler_key = UdpHandlerKey::new(
                    self.agent_connection.get_connection_id().to_string(),
                    user_token,
                    agent_address,
                    src_address,
                    dst_address,
                );
                let udp_handler = UdpHandler::new(udp_handler_key, self.agent_connection);
                udp_handler.exec(udp_raw_data).await?;
                Ok(())
            },
            PpaassMessageAgentPayloadType::DnsLookupRequest => {
                info!(
                    "Agent connection {} receive dns lookup request from agent.",
                    self.agent_connection.get_connection_id()
                );
                let dns_lookup_request: DnsLookupRequest = data.as_slice().try_into()?;
                let dns_lookup_handler_key = DnsLookupHandlerKey::new(self.agent_connection.get_connection_id().to_string(), user_token, agent_address);
                let dns_lookup_handler = DnsLookupHandler::new(dns_lookup_handler_key, self.agent_connection);
                dns_lookup_handler.exec(dns_lookup_request).await?;
                Ok(())
            },
        }
    }
}
