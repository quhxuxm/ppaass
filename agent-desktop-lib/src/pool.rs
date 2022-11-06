use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use deadpool::managed::{self, Manager};

use ppaass_io::PpaassMessageFramed;

use tokio::net::TcpStream;
use tracing::error;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};
pub(crate) struct ProxyServerConnectionPool {
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}
impl ProxyServerConnectionPool {
    pub(crate) fn new(configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Self {
        Self {
            configuration,
            rsa_crypto_fetcher,
        }
    }
}

#[async_trait]
impl Manager for ProxyServerConnectionPool {
    type Type = PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>;
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

    async fn recycle(&self, _: &mut Self::Type) -> managed::RecycleResult<Self::Error> {
        Ok(())
    }
}
