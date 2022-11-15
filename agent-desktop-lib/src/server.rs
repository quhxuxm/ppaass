use std::{sync::Arc, time::Duration};

use deadpool::managed::Pool;
use tokio::{net::TcpListener, sync::Semaphore};
use tracing::{debug, error, info};

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher};
use crate::{flow::dispatcher::FlowDispatcher, pool::ProxyMessageFramedManager};
use anyhow::{Context, Result};

pub struct AgentServer {
    configuration: Arc<AgentServerConfig>,
}

impl AgentServer {
    pub fn new(configuration: Arc<AgentServerConfig>) -> Self {
        Self { configuration }
    }

    pub async fn start(&mut self) -> Result<()> {
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!(
                "::1:{}",
                self.configuration
                    .get_port()
                    .context("can not get port from agent configuration file (ip v6)")?
            )
        } else {
            format!(
                "0.0.0.0:{}",
                self.configuration
                    .get_port()
                    .context("can not get port from agent configuration file (ip v4)")?
            )
        };
        let rsa_crypto_fetcher = Arc::new(AgentServerRsaCryptoFetcher::new(self.configuration.clone())?);

        info!("Agent server start to serve request on address: {server_bind_addr}.");
        let tcp_listener = TcpListener::bind(&server_bind_addr)
            .await
            .context("Fail to bind tcp listener for agent server")?;

        let proxy_connection_pool_builder =
            Pool::<ProxyMessageFramedManager>::builder(ProxyMessageFramedManager::new(self.configuration.clone(), rsa_crypto_fetcher.clone()));
        let proxy_connection_pool = proxy_connection_pool_builder
            .build()
            .map_err(|e| anyhow::anyhow!(e))
            .context("Fail to create proxy server connection pool.")?;
        loop {
            let (client_tcp_stream, client_socket_address) = tcp_listener.accept().await.context("Fail to accept client tcp connection because if error.")?;
            if let Err(e) = client_tcp_stream.set_nodelay(true).context("Fail to set client tcp stream to no delay") {
                error!("Fail to set no delay on client tcp connection because of error: {e:?}");
                continue;
            }
            debug!("Accept client tcp connection on address: {}", client_socket_address);

            let mut flow = match FlowDispatcher::dispatch(client_tcp_stream, client_socket_address).await {
                Err(e) => {
                    error!("Fail to dispatch client tcp connection to concrete flow because of error: {e:?}");
                    continue;
                },
                Ok(v) => v,
            };
            if let Err(e) = flow
                .as_mut()
                .exec(proxy_connection_pool.clone(), self.configuration.clone(), rsa_crypto_fetcher.clone())
                .await
            {
                error!("Fail to execute client flow because of error: {e:?}");
            };
        }
    }
}
