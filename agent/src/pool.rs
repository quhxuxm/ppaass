use std::sync::Arc;
use std::{fmt::Debug, str::FromStr};
use std::{net::SocketAddr, time::Duration};

use tokio::{net::TcpStream, time::timeout};

use tracing::{debug, error};

use ppaass_common::generate_uuid;
use ppaass_common::PpaassConnection;

use crate::{
    config::AGENT_CONFIG,
    crypto::AgentServerRsaCryptoFetcher,
    error::{AgentError, NetworkError},
};

#[derive(Debug)]
pub(crate) struct ProxyConnectionPool {
    proxy_addresses: Vec<SocketAddr>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}

impl ProxyConnectionPool {
    pub(crate) async fn new(rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Result<Self, AgentError> {
        let proxy_addresses_configuration = AGENT_CONFIG
            .get_proxy_addresses()
            .expect("Fail to parse proxy addresses from configuration file");
        let proxy_addresses: Vec<SocketAddr> = proxy_addresses_configuration
            .iter()
            .filter_map(|addr| SocketAddr::from_str(addr).ok())
            .collect::<Vec<SocketAddr>>();
        if proxy_addresses.is_empty() {
            error!("No available proxy address for runtime to use.");
            panic!("No available proxy address for runtime to use.")
        }

        Ok(Self {
            proxy_addresses,
            rsa_crypto_fetcher,
        })
    }

    pub(crate) async fn take_connection(&self) -> Result<PpaassConnection<TcpStream, AgentServerRsaCryptoFetcher, String>, AgentError> {
        debug!("Take proxy connection from pool.");
        let proxy_tcp_stream = match timeout(
            Duration::from_secs(AGENT_CONFIG.get_connect_to_proxy_timeout()),
            TcpStream::connect(self.proxy_addresses.as_slice()),
        )
        .await
        {
            Err(_) => {
                error!("Fail connect to proxy because of timeout.");
                return Err(NetworkError::Timeout(AGENT_CONFIG.get_connect_to_proxy_timeout()).into());
            },
            Ok(Ok(proxy_tcp_stream)) => proxy_tcp_stream,
            Ok(Err(e)) => {
                error!("Fail connect to proxy because of error: {e:?}");
                return Err(NetworkError::Io(e).into());
            },
        };
        debug!("Success connect to proxy.");
        proxy_tcp_stream.set_nodelay(true).map_err(NetworkError::Io)?;
        let proxy_connection = PpaassConnection::new(
            generate_uuid(),
            proxy_tcp_stream,
            self.rsa_crypto_fetcher.clone(),
            AGENT_CONFIG.get_compress(),
            AGENT_CONFIG.get_proxy_send_buffer_size(),
        );
        Ok(proxy_connection)
    }
}
