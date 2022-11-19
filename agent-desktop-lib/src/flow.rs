use async_trait::async_trait;
use std::sync::Arc;

pub(crate) mod dispatcher;
use crate::config::AgentServerConfig;
use crate::crypto::AgentServerRsaCryptoFetcher;
use crate::pool::ProxyConnectionManager;
use anyhow::Result;
use deadpool::managed::Pool;

mod http;
mod socks;

#[async_trait]
pub(crate) trait ClientFlow {
    async fn exec(
        &mut self, proxy_connection_pool: Pool<ProxyConnectionManager>, configuration: Arc<AgentServerConfig>,
        rsa_crypto_fetcher: Arc<AgentServerRsaCryptoFetcher>,
    ) -> Result<()>;
}
