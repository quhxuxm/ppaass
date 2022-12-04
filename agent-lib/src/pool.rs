use std::net::SocketAddr;
use std::sync::Arc;
use std::{fmt::Debug, str::FromStr};

use anyhow::Result;
use futures::{
    stream::{SplitSink, SplitStream},
    StreamExt,
};
use tokio::{net::TcpStream, sync::Mutex};
use tracing::{debug, error};

use ppaass_common::generate_uuid;
use ppaass_common::PpaassMessage;
use ppaass_common::PpaassMessageFramed;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};

pub(crate) type ProxyMessageFramed = PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>;
pub(crate) type ProxyMessageFramedWrite = SplitSink<ProxyMessageFramed, PpaassMessage>;
pub(crate) type ProxyMessageFramedRead = SplitStream<ProxyMessageFramed>;

pub(crate) struct ProxyConnection {
    id: String,
    read: ProxyMessageFramedRead,
    write: ProxyMessageFramedWrite,
}

impl ProxyConnection {
    pub(crate) fn get_id(&self) -> &str {
        &self.id
    }

    pub(crate) fn split(self) -> (ProxyMessageFramedRead, ProxyMessageFramedWrite) {
        (self.read, self.write)
    }
}

impl Debug for ProxyConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConnection").field("id", &self.id).finish()
    }
}

#[derive(Debug)]
pub(crate) struct ProxyConnectionPool {
    proxy_addresses: Vec<SocketAddr>,
    connections: Arc<Mutex<Vec<ProxyConnection>>>,
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
}

impl ProxyConnectionPool {
    pub(crate) async fn new(configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Result<Self> {
        let proxy_addresses_configuration = configuration
            .get_proxy_addresses()
            .expect("Fail to parse proxy addresses from configuration file");
        let mut proxy_addresses: Vec<SocketAddr> = Vec::new();
        for address in proxy_addresses_configuration {
            match SocketAddr::from_str(address) {
                Ok(r) => {
                    debug!("Put proxy server address: {:?}", r);
                    proxy_addresses.push(r);
                },
                Err(e) => {
                    error!("Fail to convert proxy address to socket address because of error: {:#?}", e);
                },
            }
        }
        if proxy_addresses.is_empty() {
            error!("No available proxy address for runtime to use.");
            panic!("No available proxy address for runtime to use.")
        }

        let connections = Arc::new(Mutex::new(Vec::new()));
        let pool = Self {
            proxy_addresses,
            connections,
            configuration: configuration.clone(),
            rsa_crypto_fetcher: rsa_crypto_fetcher.clone(),
        };
        pool.feed_connections().await?;
        Ok(pool)
    }

    async fn feed_connections(&self) -> Result<()> {
        let proxy_connection_number = self.configuration.get_proxy_connection_number();
        let connections_out = self.connections.clone();
        let mut connections = connections_out.lock().await;
        debug!("Begin to feed proxy connections");
        loop {
            let current_connection_len = connections.len();
            if current_connection_len >= proxy_connection_number {
                break;
            }
            for _ in current_connection_len..proxy_connection_number {
                let proxy_addresses = self.proxy_addresses.clone();
                let configuration = self.configuration.clone();
                let rsa_crypto_fetcher = self.rsa_crypto_fetcher.clone();
                let proxy_tcp_stream = match TcpStream::connect(proxy_addresses.as_slice()).await {
                    Ok(proxy_tcp_stream) => proxy_tcp_stream,
                    Err(e) => {
                        error!("Fail to feed proxy connection because of error.");
                        continue;
                    },
                };
                debug!("Success connect to proxy when feed connection pool.");
                let proxy_message_framed = PpaassMessageFramed::new(proxy_tcp_stream, configuration.get_compress(), 1024 * 64, rsa_crypto_fetcher.clone());
                let (proxy_message_framed_write, proxy_message_framed_read) = proxy_message_framed.split();
                let connection = ProxyConnection {
                    id: generate_uuid(),
                    read: proxy_message_framed_read,
                    write: proxy_message_framed_write,
                };
                connections.push(connection);
            }
        }
        Ok(())
    }

    pub(crate) async fn take_connection(&self) -> Result<ProxyConnection> {
        loop {
            let mut connections = self.connections.lock().await;
            let connection = connections.pop();
            match connection {
                Some(connection) => {
                    debug!("Success to take connection from pool.");
                    return Ok(connection);
                },
                None => {
                    self.feed_connections().await?;
                },
            }
        }
    }
}
