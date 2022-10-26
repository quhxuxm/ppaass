use std::{sync::Arc, time::Duration};

use clap::Parser;
use snafu::ResultExt;

use crate::error::ConfigurtionFileParseFailError;
use crate::error::IoError;
use crate::{arguments::ProxyServerArguments, config::ProxyServerConfig, constant::DEFAULT_CONFIG_FILE_PATH, error::Error, server::ProxyServer};
use tokio::{runtime::Builder, sync::mpsc::channel as tokio_mpsc_channel};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{Receiver, Sender},
};
use tracing::{debug, error, info};
#[derive(Debug)]
pub(crate) enum ProxyServerManagementCommand {
    Restart(ProxyServerConfig),
}
pub(crate) struct ProxyServerManager {
    command_sender: Sender<ProxyServerManagementCommand>,
    command_receiver: Option<Receiver<ProxyServerManagementCommand>>,
}

impl ProxyServerManager {
    pub(crate) fn new() -> Result<Self, Error> {
        let (command_sender, command_receiver) = tokio_mpsc_channel::<ProxyServerManagementCommand>(1);
        Ok(Self {
            command_sender,
            command_receiver: Some(command_receiver),
        })
    }

    async fn prepare_config(&self, arguments: &ProxyServerArguments) -> Result<ProxyServerConfig, Error> {
        let mut config_file_path = DEFAULT_CONFIG_FILE_PATH;
        if let Some(ref path) = arguments.configuration_file {
            config_file_path = path.as_str();
        }
        let config_file_content = tokio::fs::read_to_string(config_file_path).await.context(IoError {
            message: "Fail to read configuration file content.",
        })?;
        let mut result = toml::from_str::<ProxyServerConfig>(&config_file_content).context(ConfigurtionFileParseFailError { file_name: config_file_path })?;
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

    async fn start_config_monitor(&self, config_file_path: String, original_config_file_content: String) -> Result<(), Error> {
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
                    let new_proxy_server_config: ProxyServerConfig = match toml::from_str(&current_cfg_file_content) {
                        Err(e) => {
                            error!("Fail to re-generate proxy server configuration because of error: {e:?}");
                            original_config_file_content = current_cfg_file_content;
                            continue;
                        },
                        Ok(v) => v,
                    };
                    if let Err(e) = command_sender.send(ProxyServerManagementCommand::Restart(new_proxy_server_config)).await {
                        error!("Fail to send Proxy server management command (Restart) because of error: {e:?}");
                    };
                }
                original_config_file_content = current_cfg_file_content;
            }
        });
        Ok(())
    }

    async fn start_command_monitor(
        &mut self, _config: Arc<ProxyServerConfig>, mut server_runtime_builder: Builder, mut current_server_runtime: Runtime,
    ) -> Result<(), Error> {
        let mut command_receiver = self.command_receiver.take().unwrap();
        let guard = tokio::spawn(async move {
            loop {
                let command = command_receiver.recv().await;
                match command {
                    None => {
                        error!("Proxy server command channel closed.");
                        return;
                    },
                    Some(ProxyServerManagementCommand::Restart(new_proxy_server_config)) => {
                        current_server_runtime.shutdown_background();
                        current_server_runtime = match server_runtime_builder.build() {
                            Err(e) => {
                                error!("Fail to build new proxy server runtime because of error: {e:?}");
                                panic!("Fail to build new proxy server runtime because of error: {e:?}");
                            },
                            Ok(v) => v,
                        };
                        if let Err(e) = Self::start_proxy_server(Arc::new(new_proxy_server_config), &current_server_runtime).await {
                            error!("Fail to restart proxy server because of error: {e:?}");
                            panic!("Fail to restart proxy server because of error: {e:?}");
                        };
                        continue;
                    },
                }
            }
        });
        if let Err(e) = guard.await {
            error!("Error happen in proxy server monitor, error: {e:?}");
            panic!("Error happen in proxy server monitor, error: {e:?}");
        };
        Ok(())
    }

    async fn start_proxy_server(config: Arc<ProxyServerConfig>, server_runtime: &Runtime) -> Result<(), Error> {
        server_runtime.spawn(async {
            info!("Begin to start proxy server.");
            let mut proxy_server = ProxyServer::new(config);
            if let Err(e) = proxy_server.start().await {
                error!("Fail to start proxy server because of error: {e:?}");
                panic!("Fail to start proxy server because of error: {e:?}")
            }
        });
        Ok(())
    }

    pub(crate) async fn start(mut self) -> Result<(), Error> {
        let arguments = ProxyServerArguments::parse();
        let config = Arc::new(self.prepare_config(&arguments).await?);
        let mut server_runtime_builder = Builder::new_multi_thread();
        server_runtime_builder.enable_all();
        server_runtime_builder.thread_name("proxy-server-runtime");
        server_runtime_builder.worker_threads(config.get_thread_number());
        let server_runtime = server_runtime_builder.build().context(IoError {
            message: "Fail to build proxy server runtime.",
        })?;
        Self::start_proxy_server(config.clone(), &server_runtime).await?;
        self.start_command_monitor(config, server_runtime_builder, server_runtime).await?;
        Ok(())
    }
}
