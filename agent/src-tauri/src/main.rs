#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use common::LogTimer;
use config::{AgentArguments, AgentConfig, AgentLogConfig};

use crate::{
    config::{DEFAULT_AGENT_CONFIGURATION_FILE, DEFAULT_AGENT_LOG_CONFIG_FILE},
    server::AgentServer,
};

use tracing::{debug, metadata::LevelFilter, Level};
use tracing_subscriber::{fmt::Layer, prelude::__tracing_subscriber_SubscriberExt, Registry};
use ui::main_frame::MainFrame;

pub(crate) mod message {
    pub(crate) mod socks5;
}

pub(crate) mod codec;
pub(crate) mod server;

pub(crate) mod service;

pub(crate) mod config;
pub(crate) mod ui;

fn prepare_agent_config(arguments: &AgentArguments) -> AgentConfig {
    let configuration_file_content = match &arguments.configuration_file {
        None => {
            println!("Starting ppaass-agent with default configuration file:  {DEFAULT_AGENT_CONFIGURATION_FILE}");
            std::fs::read_to_string(DEFAULT_AGENT_CONFIGURATION_FILE).expect("Fail to read agent configuration file.")
        },
        Some(path) => {
            println!("Starting ppaass-agent with customized configuration file: {}", path.as_str());
            std::fs::read_to_string(path.as_str()).expect("Fail to read agent configuration file.")
        },
    };
    let mut configuration = toml::from_str::<AgentConfig>(&configuration_file_content).expect("Fail to parse agent configuration file");
    if let Some(port) = arguments.port {
        configuration.set_port(port);
    }
    if let Some(compress) = arguments.compress {
        configuration.set_compress(compress);
    }
    if let Some(client_buffer_size) = arguments.client_buffer_size {
        configuration.set_client_buffer_size(client_buffer_size)
    }
    if let Some(message_framed_buffer_size) = arguments.message_framed_buffer_size {
        configuration.set_message_framed_buffer_size(message_framed_buffer_size)
    }
    if let Some(so_backlog) = arguments.so_backlog {
        configuration.set_so_backlog(so_backlog)
    }
    configuration
}

fn main() -> Result<()> {
    let arguments = AgentArguments::parse();
    let log_configuration_file_content = std::fs::read_to_string(arguments.log_configuration_file.as_deref().unwrap_or(DEFAULT_AGENT_LOG_CONFIG_FILE))
        .expect("Fail to read agnet log configuration file.");
    let log_configuration = toml::from_str::<AgentLogConfig>(&log_configuration_file_content).expect("Fail to parse agnet log configuration file");
    let log_directory = log_configuration.log_dir().as_ref().expect("No log directory given.");
    let log_file = log_configuration.log_file().as_ref().expect("No log file name given.");
    let default_log_level = &Level::ERROR.to_string();
    let log_max_level = log_configuration.max_log_level().as_ref().unwrap_or(default_log_level);
    let file_appender = tracing_appender::rolling::daily(log_directory, log_file);
    let (non_blocking, _appender_guard) = tracing_appender::non_blocking(file_appender);
    let log_level_filter = match LevelFilter::from_str(log_max_level) {
        Err(e) => {
            panic!("Fail to initialize log because of error: {:#?}", e);
        },
        Ok(v) => v,
    };
    let subscriber = Registry::default()
        .with(
            Layer::default()
                .with_level(true)
                .with_target(true)
                .with_timer(LogTimer)
                .with_thread_ids(true)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_writer(non_blocking),
        )
        .with(log_level_filter);
    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        panic!("Fail to initialize tracing subscriber because of error: {:#?}", e);
    };
    let configuration = prepare_agent_config(&arguments);
    let agent_server = AgentServer::new()?;
    let agent_server_handler = agent_server.init();
    debug!("Begint to initialize GUI");
    let main_frame = MainFrame::new(agent_server_handler, configuration, arguments);
    main_frame.run();
    Ok(())
}
