use std::{sync::Arc, time::Duration};

use anyhow::Result;
use clap::Parser;

use crate::{arguments::ProxyServerArguments, config::ProxyServerConfig, constant::DEFAULT_CONFIG_FILE_PATH, server::ProxyServer};
use tokio::{runtime::Builder, sync::mpsc::channel as tokio_mpsc_channel};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{Receiver, Sender},
};
use tracing::{debug, error};

#[derive(Debug)]
pub(crate) enum ProxyServerManagementCommand {
    Restart,
}
pub(crate) struct ProxyServerManager {
    command_sender: Sender<ProxyServerManagementCommand>,
    command_receiver: Receiver<ProxyServerManagementCommand>,
    server_runtime: Option<Runtime>,
}

impl ProxyServerManager {
    pub(crate) fn new() -> Result<Self> {
        let (command_sender, command_receiver) = tokio_mpsc_channel::<ProxyServerManagementCommand>(1);
        Ok(Self {
            command_sender,
            command_receiver,
            server_runtime: None,
        })
    }

    async fn prepare_config(&self, arguments: &ProxyServerArguments) -> Result<ProxyServerConfig> {
        let mut config_file_path = DEFAULT_CONFIG_FILE_PATH;
        if let Some(ref path) = arguments.configuration_file {
            config_file_path = path.as_str();
        }
        let config_file_content = tokio::fs::read_to_string(config_file_path).await?;
        let mut result = toml::from_str::<ProxyServerConfig>(&config_file_content)?;
        if let Some(port) = arguments.port {
            result.set_port(port);
        }
        if let Some(compress) = arguments.compress {
            result.set_compress(compress);
        }
        if let Some(ref dir) = arguments.rsa_dir {
            result.set_rsa_dir(dir.as_str());
        }
        self.start_config_monitor(config_file_path.to_owned(), config_file_content).await?;
        Ok(result)
    }

    async fn start_config_monitor(&self, config_file_path: String, original_config_file_content: String) -> Result<()> {
        let command_sender = self.command_sender.clone();
        tokio::spawn(async move {
            let mut original_config_file_content = original_config_file_content;
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            loop {
                interval.tick().await;
                let current_cfg_file_content = match tokio::fs::read_to_string(&config_file_path).await {
                    Err(e) => {
                        debug!("Fail to read proxy server configuration file because of error: {e:?}");
                        continue;
                    },
                    Ok(v) => v,
                };
                if !current_cfg_file_content.eq(&original_config_file_content) {
                    if let Err(e) = command_sender.send(ProxyServerManagementCommand::Restart).await {
                        error!("Fail to send Proxy server management command (Restart) because of error: {e:?}");
                    };
                }
                original_config_file_content = current_cfg_file_content;
            }
        });
        Ok(())
    }
    async fn start_command_monitor(&mut self, config: Arc<ProxyServerConfig>) -> Result<()> {
        loop {
            let command = self.command_receiver.recv().await;
            match command {
                None => {
                    error!("Proxy server command channel closed.");
                    return Ok(());
                },
                Some(ProxyServerManagementCommand::Restart) => {
                    if let Some(server_runtime) = self.server_runtime.take() {
                        server_runtime.shutdown_timeout(Duration::from_secs(10));
                    }
                    self.start_proxy_server(config.clone()).await?;
                    continue;
                },
            }
        }
    }

    async fn start_proxy_server(&mut self, config: Arc<ProxyServerConfig>) -> Result<()> {
        let mut server_runtime_builder = Builder::new_multi_thread();
        server_runtime_builder.enable_all();
        server_runtime_builder.thread_name("proxy-server-runtime");
        server_runtime_builder.worker_threads(config.get_thread_number());
        let server_runtime = server_runtime_builder.build()?;
        server_runtime.spawn(async {
            let mut proxy_server = ProxyServer::new(config);
            if let Err(e) = proxy_server.start().await {
                error!("Fail to start proxy server because of error: {e:?}");
            }
        });
        self.server_runtime = Some(server_runtime);
        Ok(())
    }

    pub(crate) async fn start(mut self) -> Result<()> {
        let arguments = ProxyServerArguments::parse();
        let config = Arc::new(self.prepare_config(&arguments).await?);
        self.start_command_monitor(config.clone()).await?;
        self.start_proxy_server(config).await?;
        Ok(())
    }
}
