use std::{sync::Arc, time::Duration};

use snafu::ResultExt;
use tokio::{net::TcpListener, sync::Semaphore};
use tracing::{debug, error, info};

use crate::error::AcceptClientTcpConnectionError;
use crate::error::IoError;
use crate::{config::AgentServerConfig, crypto::AgentServerRsaCryptoFetcher, error::Error};

pub(crate) struct AgentServer {
    configuration: Arc<AgentServerConfig>,
    client_tcp_connection_accept_semaphore: Arc<Semaphore>,
}

impl AgentServer {
    pub(crate) fn new(configuration: Arc<AgentServerConfig>) -> Self {
        Self {
            configuration,
            client_tcp_connection_accept_semaphore: Arc::new(Semaphore::new(configuration.get_client_max_connection_number())),
        }
    }

    pub(crate) async fn start(&mut self) -> Result<(), Error> {
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!("::1:{}", self.configuration.get_port()?)
        } else {
            format!("0.0.0.0:{}", self.configuration.get_port()?)
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
                    error!("{}", AcceptClientTcpConnectionError { message: e.to_string() }.build());
                    continue;
                },
                Err(e) => {
                    error!("{}", AcceptClientTcpConnectionError { message: e.to_string() }.build());
                    continue;
                },
            };
            let (client_tcp_stream, client_socket_address) = match tcp_listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    error!("{}", AcceptClientTcpConnectionError { message: e.to_string() }.build());
                    continue;
                },
            };
            client_tcp_stream.set_nodelay(true).context(IoError { message: "set no delay fail" })?;
            debug!("Accept client tcp connection on address: {}", client_socket_address);
            let rsa_crypto_fetcher = rsa_crypto_fetcher.clone();
            let configuration = self.configuration.clone();
            let agent_message_framed = match AgentMessageFramed::new(
                agent_tcp_stream,
                self.configuration.get_compress(),
                client_connection_buffer_size,
                proxy_server_rsa_crypto_fetcher.clone(),
            ) {
                Ok(v) => v,
                Err(e) => {
                    error!("{e}");
                    drop(agent_tcp_connection_accept_permit);
                    continue;
                },
            };
            let transport = Transport::new(agent_message_framed, configuration, agent_tcp_connection_accept_permit);
            transport.exec().await;
        }
    }
}
