use std::sync::Arc;

use async_trait::async_trait;
use deadpool::managed::{self, Manager};
use ppaass_io::PpaassMessageFramed;
use tokio::net::TcpStream;

use crate::{error::Error, config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};

pub(crate) struct ProxyServerConnectionPool{
    configuration: Arc<AgentServerConfig>,
    rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>
};

#[async_trait]
impl Manager for ProxyServerConnectionPool {
    type Type = PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>;
    type Error = Error;

    async fn create(&self) -> Result<PpaassMessageFramed<TcpStream, AgentServerRsaCryptoFetcher>, Error> {
        let proxy_tcp_stream = TcpStream::connect(addr).await;
        PpaassMessageFramed::new(stream, compress, buffer_size, rsa_crypto_fetcher);
        Ok(Computer {})
    }

    async fn recycle(&self, _: &mut TcpStream) -> managed::RecycleResult<Error> {
        Ok(())
    }
}
