use std::{sync::Arc, time::Duration};

use crate::error::IoError;
use crate::{common::AgentMessageFramed, config::ProxyServerConfig, crypto::ProxyServerRsaCryptoFetcher, error::Error, transport::Transport};
use snafu::{Backtrace, GenerateImplicitData, ResultExt};
use tokio::{io::AsyncWriteExt, net::TcpListener, sync::Semaphore};
use tracing::{error, info};

pub(crate) struct ProxyServer {
    configuration: Arc<ProxyServerConfig>,
    agent_connection_number: Arc<Semaphore>,
}

impl ProxyServer {
    pub(crate) fn new(configuration: Arc<ProxyServerConfig>) -> Self {
        let agent_max_connection_number = configuration.get_agent_max_connection_number();
        Self {
            configuration,
            agent_connection_number: Arc::new(Semaphore::new(agent_max_connection_number)),
        }
    }

    pub(crate) async fn start(&mut self) -> Result<(), Error> {
        let server_bind_addr = if self.configuration.get_ipv6() {
            format!("::1:{}", self.configuration.get_port())
        } else {
            format!("0.0.0.0:{}", self.configuration.get_port())
        };
        let proxy_server_rsa_crypto_fetcher = Arc::new(ProxyServerRsaCryptoFetcher::new(self.configuration.clone())?);
        let agent_connection_buffer_size = self.configuration.get_agent_connection_buffer_size();
        info!("Proxy server start to serve request on address: {server_bind_addr}.");
        let tcp_listener = TcpListener::bind(&server_bind_addr).await.context(IoError {
            message: "Fail to bind tcp listener for proxy server",
        })?;
        loop {
            let (mut agent_tcp_stream, agent_socket_address) = match tcp_listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    error!("Fail to accept agent tcp connection because of error.");
                    error!("{}", Backtrace::generate_with_source(&e));
                    continue;
                },
            };
            let agent_connection_number = self.agent_connection_number.clone();
            let agent_tcp_connection_accept_permit = match tokio::time::timeout(
                Duration::from_secs(self.configuration.get_agent_tcp_connection_accept_timout_seconds()),
                agent_connection_number.clone().acquire_owned(),
            )
            .await
            {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    error!("Fail to accept agent tcp connection [{agent_socket_address:?}] because of error happen when acquire agent tcp connection accept permit.");
                    error!("{}", Backtrace::generate_with_source(&e));
                    if let Err(e) = agent_tcp_stream.shutdown().await {
                        error!("Fail to shutdown agent tcp stream because of error.");
                        error!("{}", Backtrace::generate_with_source(&e));
                    }
                    continue;
                },
                Err(e) => {
                    error!(
                        "Fail to accept agent tcp connection [{agent_socket_address:?}] because of timeout when acquire agent tcp connection accept permit."
                    );
                    error!("{}", Backtrace::generate_with_source(&e));
                    if let Err(e) = agent_tcp_stream.shutdown().await {
                        error!("Fail to shutdown agent tcp stream because of error.");
                        error!("{}", Backtrace::generate_with_source(&e));
                    }
                    continue;
                },
            };
            let proxy_server_rsa_crypto_fetcher = proxy_server_rsa_crypto_fetcher.clone();
            let configuration = self.configuration.clone();
            let agent_message_framed = match AgentMessageFramed::new(
                agent_tcp_stream,
                self.configuration.get_compress(),
                agent_connection_buffer_size,
                proxy_server_rsa_crypto_fetcher.clone(),
            ) {
                Ok(v) => v,
                Err(e) => {
                    error!("Fail to handle agent tcp connection because of error.");
                    error!("{}", Backtrace::generate_with_source(&e));
                    drop(agent_tcp_connection_accept_permit);
                    continue;
                },
            };
            let transport = Transport::new(agent_message_framed, configuration, agent_tcp_connection_accept_permit);
            transport.exec().await;
        }
    }
}
