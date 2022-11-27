use std::net::SocketAddr;
use std::sync::Arc;
use std::{fmt::Debug, str::FromStr};

use anyhow::Context;
use async_trait::async_trait;
use deadpool::managed::{Manager, Object, Pool, RecycleResult};
use futures::{
    stream::{SplitSink, SplitStream},
    StreamExt,
};

use ppaass_common::generate_uuid;
use ppaass_io::PpaassMessageFramed;

use anyhow::Result;
use ppaass_protocol::PpaassMessage;
use tokio::{net::TcpStream, sync::Mutex};
use tracing::{debug, error};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};

pub(crate) type ProxyMessageFramed = PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>;
pub(crate) type ProxyMessageFramedWriter = SplitSink<ProxyMessageFramed, PpaassMessage>;
pub(crate) type ProxyMessageFramedReader = SplitStream<ProxyMessageFramed>;
pub(crate) type PooledProxyConnection = Object<ProxyConnectionManager>;
pub(crate) type ProxyConnectionPool = Pool<ProxyConnectionManager>;

#[derive(Debug)]
pub(crate) struct PooledProxyConnectionError {
    pub(crate) pooled_proxy_connection: Option<PooledProxyConnection>,
    pub(crate) source: anyhow::Error,
}

pub(crate) struct ProxyConnection {
    id: String,
    reader: Arc<Mutex<ProxyMessageFramedReader>>,
    writer: Arc<Mutex<ProxyMessageFramedWriter>>,
}

impl ProxyConnection {
    pub(crate) fn get_id(&self) -> &String {
        &self.id
    }
    pub(crate) fn clone_reader(&self) -> Arc<Mutex<ProxyMessageFramedReader>> {
        self.reader.clone()
    }

    pub(crate) fn clone_writer(&self) -> Arc<Mutex<ProxyMessageFramedWriter>> {
        self.writer.clone()
    }
}

impl Debug for ProxyConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyConnection").field("id", &self.id).finish()
    }
}

#[derive(Debug)]
pub(crate) struct ProxyConnectionManager {
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    proxy_addresses: Vec<SocketAddr>,
}

impl ProxyConnectionManager {
    pub(crate) fn new(configuration: Arc<AgentServerConfig>, rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>) -> Self {
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
        Self {
            configuration,
            rsa_crypto_fetcher,
            proxy_addresses,
        }
    }
}

#[async_trait]
impl Manager for ProxyConnectionManager {
    type Type = ProxyConnection;
    type Error = anyhow::Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        let proxy_tcp_stream = TcpStream::connect(self.proxy_addresses.as_slice()).await?;
        let ppaass_message_framed = PpaassMessageFramed::new(proxy_tcp_stream, self.configuration.get_compress(), 1024 * 64, self.rsa_crypto_fetcher.clone())
            .context("fail to create ppaass message framed")?;
        let (ppaass_message_framed_write, ppaass_message_framed_read) = ppaass_message_framed.split();
        Ok(ProxyConnection {
            id: generate_uuid(),
            reader: Arc::new(Mutex::new(ppaass_message_framed_read)),
            writer: Arc::new(Mutex::new(ppaass_message_framed_write)),
        })
    }

    async fn recycle(&self, proxy_connection: &mut Self::Type) -> RecycleResult<Self::Error> {
        debug!("Proxy connection [{}] recycled.", proxy_connection.id);
        Ok(())
    }

    fn detach(&self, proxy_connection: &mut Self::Type) {
        debug!("Proxy connection [{}] detached.", proxy_connection.id);
    }
}
