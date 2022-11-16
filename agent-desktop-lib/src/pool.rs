use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use deadpool::managed::{self, Manager, Pool};
use futures::SinkExt;

use ppaass_common::generate_uuid;
use ppaass_io::PpaassMessageFramed;

use anyhow::Result;
use ppaass_protocol::{PpaassMessagePayloadEncryptionSelector, PpaassMessageUtil};
use tokio::{net::TcpStream, task::JoinHandle};
use tracing::error;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, AgentServerPayloadEncryptionTypeSelector};

pub(crate) type ProxyMessageFramed = PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>;

pub(crate) struct ProxyMessageFramedManager {
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}

impl ProxyMessageFramedManager {
    pub(crate) fn new(configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Self {
        Self {
            configuration,
            rsa_crypto_fetcher,
        }
    }
}

#[async_trait]
impl Manager for ProxyMessageFramedManager {
    type Type = ProxyMessageFramed;
    type Error = anyhow::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let proxy_addresses_configuration = self
            .configuration
            .get_proxy_addresses()
            .ok_or(anyhow::anyhow!(format!("fail to parse proxy addresses from configuration file")))?;
        let mut proxy_addresses: Vec<SocketAddr> = Vec::new();
        for address in proxy_addresses_configuration {
            match SocketAddr::from_str(&address) {
                Ok(r) => {
                    println!("Put proxy server address: {:?}", r);
                    proxy_addresses.push(r);
                },
                Err(e) => {
                    error!("Fail to convert proxy address to socket address because of error: {:#?}", e)
                },
            }
        }
        if proxy_addresses.is_empty() {
            error!("no available proxy address for runtime to use.");
            return Err(anyhow::anyhow!("no available proxy address for runtime to use."));
        }
        let proxy_tcp_stream = TcpStream::connect(&proxy_addresses.as_slice()).await?;
        PpaassMessageFramed::new(proxy_tcp_stream, self.configuration.get_compress(), 1024 * 64, self.rsa_crypto_fetcher.clone())
            .context("fail to create ppaass message framed")
    }

    async fn recycle(&self, _proxy_message_framed: &mut Self::Type) -> managed::RecycleResult<Self::Error> {
        Ok(())
    }
}

pub(crate) struct ProxyMessageFramedPoolKeepalive {
    proxy_connection_pool: Pool<ProxyMessageFramedManager>,
    configuration: Arc<AgentServerConfig>,
}

impl ProxyMessageFramedPoolKeepalive {
    pub(crate) fn new(proxy_connection_pool: Pool<ProxyMessageFramedManager>, configuration: Arc<AgentServerConfig>) -> Self {
        Self {
            proxy_connection_pool,
            configuration,
        }
    }
    pub(crate) fn start(self) -> Result<JoinHandle<Result<()>>> {
        let connection_pool = self.proxy_connection_pool;
        let configuration = self.configuration;
        let keepalive_guard = tokio::spawn(async move {
            loop {
                let mut connection = connection_pool.get().await.map_err(|e| anyhow::anyhow!(e))?;
                let user_token = configuration
                    .get_user_token()
                    .as_ref()
                    .context("Can not get user token from configuration file")?;
                let payload_encryption = AgentServerPayloadEncryptionTypeSelector::select(&user_token, Some(generate_uuid().into_bytes()));
                let heartbeat_message = PpaassMessageUtil::create_agent_heartbeat_request(user_token, payload_encryption)?;
                connection.send(heartbeat_message).await?;
            }
        });
        Ok(keepalive_guard)
    }
}
