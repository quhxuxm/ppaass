use std::sync::{
    mpsc::{channel as std_mpsc_channel, Receiver, Sender},
    Arc,
};

use anyhow::Result;
use clap::Parser;
use hotwatch::Hotwatch;

use tokio::runtime::{Builder, Runtime};
use tracing::{error, info};

use crate::{arguments::ProxyServerArguments, config::ProxyServerConfig, constant::DEFAULT_CONFIG_FILE_PATH, server::ProxyServer};

pub(crate) enum ProxyServerManagementCommand {
    Restart,
    Shutdown,
    Start,
}
pub(crate) struct ProxyServerManager {
    manager_runtime: Option<Runtime>,
    command_sender: Sender<ProxyServerManagementCommand>,
    command_receiver: Receiver<ProxyServerManagementCommand>,
    server: Option<ProxyServer>,
}

impl ProxyServerManager {
    pub(crate) fn new() -> Result<Self> {
        let mut manager_runtime_builder = Builder::new_current_thread();
        let manager_runtime = manager_runtime_builder.build()?;
        let (command_sender, command_receiver) = std_mpsc_channel::<ProxyServerManagementCommand>();
        Ok(Self {
            manager_runtime: Some(manager_runtime),
            command_sender,
            command_receiver,
            server: None,
        })
    }

    fn prepare_config(&self, arguments: &ProxyServerArguments) -> Result<ProxyServerConfig> {
        let mut cfg_file_path = DEFAULT_CONFIG_FILE_PATH;
        if let Some(ref path) = arguments.configuration_file {
            cfg_file_path = path.as_str();
        }
        let cfg_file_content = std::fs::read_to_string(cfg_file_path)?;
        let mut result = toml::from_str::<ProxyServerConfig>(&cfg_file_content)?;
        if let Some(port) = arguments.port {
            result.set_port(port);
        }
        if let Some(compress) = arguments.compress {
            result.set_compress(compress);
        }
        if let Some(ref dir) = arguments.rsa_dir {
            result.set_rsa_dir(dir.as_str());
        }
        let mut cfg_file_watcher = Hotwatch::new()?;
        let command_sender = self.command_sender.clone();
        cfg_file_watcher.watch(cfg_file_path, move |event| {
            info!("Proxy server configuration file has been changed by event: {event:?}");
            if let Err(e) = command_sender.send(ProxyServerManagementCommand::Restart) {
                error!("Fail to send Proxy server management command (Restart) because of error: {e:?}");
            };
        })?;
        Ok(result)
    }

    fn start_command_monitor(mut self, config: Arc<ProxyServerConfig>) {
        let manager_runtime = self.manager_runtime.take().unwrap();
        manager_runtime.spawn_blocking::<_, Result<(), anyhow::Error>>(move || loop {
            let command = self.command_receiver.recv()?;
            match command {
                ProxyServerManagementCommand::Restart | ProxyServerManagementCommand::Start => {
                    if let Some(proxy_server) = self.server.take() {
                        proxy_server.shutdown();
                    }
                    self.start_proxy_server(config.clone())?;
                    continue;
                },
                ProxyServerManagementCommand::Shutdown => {
                    if let Some(proxy_server) = self.server.take() {
                        proxy_server.shutdown();
                    }
                    continue;
                },
            }
        });
    }

    fn start_proxy_server(&mut self, config: Arc<ProxyServerConfig>) -> Result<()> {
        let proxy_server = ProxyServer::new(config)?;
        proxy_server.start()?;
        self.server = Some(proxy_server);
        Ok(())
    }

    pub(crate) fn start(mut self) -> Result<()> {
        let arguments = ProxyServerArguments::parse();
        let config = Arc::new(self.prepare_config(&arguments)?);
        self.start_proxy_server(config.clone())?;
        self.start_command_monitor(config);
        Ok(())
    }
}
