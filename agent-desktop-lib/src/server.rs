use std::{sync::Arc, time::Duration};

use snafu::{ErrorCompat, OptionExt, ResultExt};
use tokio::{net::TcpListener, sync::Semaphore};
use tracing::{debug, error, info};

use crate::error::IoError;

use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::Error};
use crate::{error::ConfigurationItemMissedError, flow::dispatcher::FlowDispatcher};

pub(crate) struct AgentServer {
    configuration: Arc<AgentServerConfig>,
    client_tcp_connection_accept_semaphore: Arc<Semaphore>,
}

impl AgentServer {
    pub(crate) fn new(configuration: Arc<AgentServerConfig>) -> Self {
        let client_max_connection_number = configuration.get_client_max_connection_number();
        Self {
            configuration,
            client_tcp_connection_accept_semaphore: Arc::new(Semaphore::new(client_max_connection_number)),
        }
    }

    pub(crate) async fn start(&mut self) -> Result<(), Error> {
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!(
                "::1:{}",
                self.configuration.get_port().context(ConfigurationItemMissedError { message: "port(ip v6)" })?
            )
        } else {
            format!(
                "0.0.0.0:{}",
                self.configuration.get_port().context(ConfigurationItemMissedError { message: "port(ip v4)" })?
            )
        };
        let rsa_crypto_fetcher = Arc::new(AgentServerRsaCryptoFetcher::new(self.configuration.clone())?);

        info!("Agent server start to serve request on address: {server_bind_addr}.");
        let tcp_listener = TcpListener::bind(&server_bind_addr).await.context(IoError {
            message: "Fail to bind tcp listener for agent server",
        })?;
        loop {
            let client_tcp_connection_accept_semaphore = self.client_tcp_connection_accept_semaphore.clone();
            let client_tcp_connection_accept_permit = match tokio::time::timeout(
                Duration::from_secs(self.configuration.get_client_tcp_connection_accept_timout_seconds()),
                client_tcp_connection_accept_semaphore.acquire_owned(),
            )
            .await
            {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    error!("Fail to accept client tcp connection because of error: {e:?}");
                    continue;
                },
                Err(e) => {
                    error!("Fail to accept client tcp connection because of error: {e:?}");
                    continue;
                },
            };
            let (client_tcp_stream, client_socket_address) = match tcp_listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    error!("Fail to accept client tcp connection because of error: {e:?}");
                    continue;
                },
            };
            if let Err(e) = client_tcp_stream.set_nodelay(true) {
                error!("Fail to accept client tcp connection because of error: {e:?}");
                continue;
            }
            debug!("Accept client tcp connection on address: {}", client_socket_address);
            let rsa_crypto_fetcher = rsa_crypto_fetcher.clone();
            let configuration = self.configuration.clone();
            let mut flow = match FlowDispatcher::dispatch(client_tcp_stream, configuration, rsa_crypto_fetcher).await {
                Err(e) => {
                    error!("Fail to dispatch client tcp connection to concrete flow because of error");
                    if let Some(error_backtrace) = ErrorCompat::backtrace(&e) {
                        error!("{}", error_backtrace);
                    }
                    continue;
                },
                Ok(v) => v,
            };
            if let Err(e) = flow.as_mut().exec().await {
                error!("Fail to execute client flow because of error: {e:?}");
            };
        }
    }
}
