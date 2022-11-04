use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use deadpool::managed::{self, Manager};
use ppaass_io::PpaassMessageFramed;
use tokio::net::TcpStream;
use tracing::error;

use crate::error::ConfigurationItemMissedError;
use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::Error};
pub(crate) struct ProxyServerConnectionPool {
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}

#[async_trait]
impl Manager for ProxyServerConnectionPool {
    type Type = PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>;
    type Error = Error;

    async fn create(&self) -> Result<PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>, Error> {
        let proxy_addresses_configuration = self
            .configuration
            .get_proxy_addresses()
            .ok_or(ConfigurationItemMissedError { message: "proxy addresses" }.build())?;
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
            error!("No available proxy address for runtime to use.");
            if let Err(e) = command_result_sender.send(Err(anyhow!("No available proxy address for runtime to use."))) {
                error!("Fail to send command result because of error:{:#?}", e)
            };
            return;
        }
        let proxy_addresses = Arc::new(proxy_addresses);
        let proxy_tcp_stream = TcpStream::connect(&proxy_addresses.as_slice()).await?;
        PpaassMessageFramed::new(proxy_tcp_stream, compress, buffer_size, rsa_crypto_fetcher);
        Ok(Computer {})
    }

    async fn recycle(&self, _: &mut TcpStream) -> managed::RecycleResult<Error> {
        Ok(())
    }
}
