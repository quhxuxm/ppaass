use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{debug, error, info};

use crate::processor::dispatcher::ClientConnectionProcessorDispatcher;
use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, pool::ProxyConnectionPool};
use anyhow::{anyhow, Context, Result};

pub struct AgentServer {
    configuration: Arc<AgentServerConfig>,
}

impl AgentServer {
    pub fn new(configuration: Arc<AgentServerConfig>) -> Self {
        Self { configuration }
    }

    pub async fn start(&mut self) -> Result<()> {
        let agent_server_bind_addr = if self.configuration.get_ipv6() {
            format!(
                "::1:{}",
                self.configuration
                    .get_port()
                    .context("Can not get port from agent configuration file (ip v6)")?
            )
        } else {
            format!(
                "0.0.0.0:{}",
                self.configuration
                    .get_port()
                    .context("Can not get port from agent configuration file (ip v4)")?
            )
        };
        let rsa_crypto_fetcher = Arc::new(AgentServerRsaCryptoFetcher::new(self.configuration.clone())?);

        info!("Agent server start to serve request on address: {agent_server_bind_addr}.");

        let agent_server_tcp_listener = TcpListener::bind(&agent_server_bind_addr)
            .await
            .context("Fail to bind tcp listener for agent server")?;
        let proxy_connection_pool = Arc::new(ProxyConnectionPool::new(self.configuration.clone(), rsa_crypto_fetcher.clone()).await?);
        loop {
            let (client_tcp_stream, client_socket_address) = match agent_server_tcp_listener
                .accept()
                .await
                .context("Fail to accept client tcp connection because if error.")
            {
                Ok(v) => v,
                Err(e) => {
                    error!("Fail to accept client connection because of error: {e:?}");
                    return Err(anyhow!(e));
                },
            };
            if let Err(e) = client_tcp_stream.set_nodelay(true).context("Fail to set client tcp stream to no delay") {
                error!("Fail to set no delay on client tcp connection because of error: {e:?}");
                continue;
            }
            debug!("Accept client tcp connection on address: {}", client_socket_address);
            let configuration = self.configuration.clone();
            let proxy_connection_pool = proxy_connection_pool.clone();
            tokio::spawn(async move {
                let processor_dispatcher = match ClientConnectionProcessorDispatcher::dispatch(client_tcp_stream, client_socket_address).await {
                    Err(e) => {
                        error!(
                            "Client tcp connection [{client_socket_address}] fail to dispatch client tcp connection to concrete flow because of error: {e:?}"
                        );
                        return;
                    },
                    Ok(v) => v,
                };
                if let Err(e) = processor_dispatcher.exec(proxy_connection_pool, configuration).await {
                    error!("Client tcp connection [{client_socket_address}] fail to execute client flow because of error: {e:?}");
                    return;
                };
                debug!("Client tcp connection [{client_socket_address}] complete to serve.")
            });
        }
    }
}
