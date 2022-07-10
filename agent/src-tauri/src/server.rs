use std::sync::{mpsc::Receiver, Arc};
use std::time::Duration;
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::mpsc::channel,
};
use std::{str::FromStr, sync::mpsc::Sender};

use anyhow::Result;
use tokio::net::TcpSocket;
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime};

use tracing::error;

use crate::service::{common::ClientConnection, AgentRsaCryptoFetcher};
use crate::{config::AgentConfig, service::pool::ProxyConnectionPool};

const DEFAULT_SERVER_PORT: u16 = 10080;

#[derive(Debug)]
pub(crate) enum AgentServerCommand {
    Start,
    Stop,
}
#[derive(Debug)]
pub(crate) struct AgentServer {
    configuration: Arc<AgentConfig>,
    command_receiver: Receiver<AgentServerCommand>,
    command_sender: Sender<AgentServerCommand>,
}

pub(crate) struct AgentServerHandler {
    command_sender: Sender<AgentServerCommand>,
}

impl AgentServerHandler {
    pub(crate) fn stop(&self) -> Result<()> {
        if let Err(e) = self.command_sender.send(AgentServerCommand::Stop) {
            error!("Fail to send stop command because of error: {e:#?}")
        };
        Ok(())
    }
    pub(crate) fn start(&self) -> Result<()> {
        if let Err(e) = self.command_sender.send(AgentServerCommand::Start) {
            error!("Fail to send start command because of error:{e:#?}")
        };
        Ok(())
    }
}

impl AgentServer {
    pub(crate) fn new(configuration: Arc<AgentConfig>) -> Result<Self> {
        let (command_sender, command_receiver) = channel::<AgentServerCommand>();
        Ok(Self {
            configuration,
            command_receiver,
            command_sender,
        })
    }

    pub(crate) fn init(self) -> AgentServerHandler {
        let configuration = self.configuration.clone();

        let command_receiver = self.command_receiver;
        let configuration = configuration.clone();

        std::thread::spawn(move || {
            let mut _runtime_holder: Option<Runtime> = None;
            loop {
                println!("Waiting for command.");
                let command = command_receiver.recv();
                println!("Receive a command.");

                match command {
                    Err(e) => {
                        eprintln!("Error happen when waiting for command: {:#?}", e);
                        if let Some(runtime) = _runtime_holder {
                            runtime.shutdown_background();
                        }
                        _runtime_holder = None;
                        continue;
                    },
                    Ok(AgentServerCommand::Stop) => {
                        println!("Receive stop command in main loop");
                        if let Some(runtime) = _runtime_holder {
                            runtime.shutdown_background();
                        }
                        _runtime_holder = None;
                        continue;
                    },
                    Ok(AgentServerCommand::Start) => {
                        println!("Receive start command in main loop");
                        let mut runtime_builder = TokioRuntimeBuilder::new_multi_thread();
                        runtime_builder
                            .enable_all()
                            .thread_keep_alive(Duration::from_secs(configuration.thread_timeout().unwrap_or(2)))
                            .max_blocking_threads(configuration.max_blocking_threads().unwrap_or(32))
                            .worker_threads(configuration.thread_number().unwrap_or(1024));
                        let runtime = match runtime_builder.build() {
                            Err(e) => {
                                error!("Fail to convert proxy address to socket address because of error: {:#?}", e);
                                return;
                            },
                            Ok(v) => v,
                        };

                        println!("Spwan a task for agent server");
                        let configuration = configuration.clone();
                        runtime.spawn(async move {
                            println!("Agent server runtime begin to initialize.");
                            let proxy_addresses_from_config = configuration.proxy_addresses().as_ref().expect("No proxy addresses configuration item");
                            let mut proxy_addresses: Vec<SocketAddr> = Vec::new();
                            for address in proxy_addresses_from_config {
                                match SocketAddr::from_str(address) {
                                    Ok(r) => {
                                        proxy_addresses.push(r);
                                    },
                                    Err(e) => {
                                        error!("Fail to convert proxy address to socket address because of error: {:#?}", e)
                                    },
                                }
                            }
                            let proxy_addresses = Arc::new(proxy_addresses);
                            let server_socket = match TcpSocket::new_v4() {
                                Err(e) => {
                                    error!("Fail to create tcp server socket because of error: {e:#?}");
                                    return;
                                },
                                Ok(v) => v,
                            };
                            if let Err(e) = server_socket.set_reuseaddr(true) {
                                error!("Fail to set tcp server socket to reuse address because of error: {e:#?}");
                                return;
                            };
                            if let Some(so_recv_buffer_size) = configuration.so_recv_buffer_size() {
                                if let Err(e) = server_socket.set_recv_buffer_size(so_recv_buffer_size) {
                                    error!("Fail to set tcp server socket recv_buffer_size because of error: {e:#?}");
                                    return;
                                };
                            }
                            if let Some(so_send_buffer_size) = configuration.so_send_buffer_size() {
                                if let Err(e) = server_socket.set_send_buffer_size(so_send_buffer_size) {
                                    error!("Fail to set tcp server socket send_buffer_size because of error: {e:#?}");
                                    return;
                                };
                            }
                            let local_socket_address = SocketAddr::V4(SocketAddrV4::new(
                                Ipv4Addr::new(0, 0, 0, 0),
                                configuration.port().unwrap_or(DEFAULT_SERVER_PORT),
                            ));
                            if let Err(e) = server_socket.bind(local_socket_address) {
                                error!("Fail to bind tcp server socket on [{local_socket_address}] because of error: {e:#?}");
                                return;
                            };
                            let listener = match server_socket.listen(configuration.so_backlog().unwrap_or(1024)) {
                                Err(e) => {
                                    error!("Fail to listen tcp server socket because of error: {e:#?}");
                                    return;
                                },
                                Ok(v) => v,
                            };
                            let agent_rsa_crypto_fetcher = match AgentRsaCryptoFetcher::new(configuration.clone()) {
                                Err(e) => {
                                    error!("Fail to generate rsa crypto fetcher because of error: {e:#?}");
                                    return;
                                },
                                Ok(v) => v,
                            };
                            let agent_rsa_crypto_fetcher = Arc::new(agent_rsa_crypto_fetcher);
                            let proxy_connection_pool = Arc::new(
                                match ProxyConnectionPool::new(proxy_addresses.clone(), configuration.clone(), agent_rsa_crypto_fetcher.clone()).await {
                                    Err(e) => {
                                        error!("Fail to generate rsa crypto fetcher because of error: {e:#?}");
                                        return;
                                    },
                                    Ok(v) => v,
                                },
                            );
                            println!("ppaass-agent is listening port: {} ", local_socket_address.port());
                            loop {
                                let agent_rsa_crypto_fetcher = agent_rsa_crypto_fetcher.clone();
                                let (client_stream, client_address) = match listener.accept().await {
                                    Err(e) => {
                                        error!("Fail to accept client connection because of error: {:#?}", e);
                                        continue;
                                    },
                                    Ok((client_stream, client_address)) => (client_stream, client_address),
                                };
                                if let Err(e) = client_stream.set_nodelay(true) {
                                    error!("Fail to set client connection no delay because of error: {:#?}", e);
                                    continue;
                                }
                                if let Some(so_linger) = configuration.client_stream_so_linger() {
                                    if let Err(e) = client_stream.set_linger(Some(Duration::from_secs(so_linger))) {
                                        error!("Fail to set client connection linger because of error: {:#?}", e);
                                    }
                                }

                                let configuration = configuration.clone();
                                let proxy_connection_pool = proxy_connection_pool.clone();
                                tokio::spawn(async move {
                                    let client_connection = ClientConnection::new(client_stream, client_address);
                                    if let Err(e) = client_connection
                                        .exec(agent_rsa_crypto_fetcher.clone(), configuration.clone(), proxy_connection_pool)
                                        .await
                                    {
                                        error!("Error happen when handle client connection [{}], error:{:#?}", client_address, e);
                                    }
                                });
                            }
                        });
                        _runtime_holder = Some(runtime);
                    },
                };
            }
        });

        AgentServerHandler {
            command_sender: self.command_sender,
        }
    }
}
