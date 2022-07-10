#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::{
    io::Read,
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use clap::Parser;
use common::LogTimer;
use config::{AgentArguments, AgentConfig, AgentLogConfig};

use server::AgentServerHandler;
use tauri::{Manager, PhysicalSize, State, SystemTray};
use tracing::{debug, error, metadata::LevelFilter, Level};
use tracing_subscriber::{fmt::Layer, prelude::__tracing_subscriber_SubscriberExt, Registry};

use crate::server::AgentServer;

pub(crate) mod message {
    pub(crate) mod socks5;
}

pub(crate) mod codec;
pub(crate) mod server;

pub(crate) mod service;

pub(crate) mod config;

const AGNT_LOG_CONFIG_FILE: &str = "ppaass-agent-log.toml";

fn init_configuration(arguments: &AgentArguments) -> AgentConfig {
    let configuration_file_content = match &arguments.configuration_file {
        None => {
            println!("Starting ppaass-agent with default configuration file:  ppaass-agent.toml");
            std::fs::read_to_string("ppaass-agent.toml").expect("Fail to read agent configuration file.")
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

#[tauri::command]
fn start_agent_server(window_state: State<'_, AgentWindowState>) {
    println!("Begin to start agent server");

    match window_state.agent_server_handler.lock() {
        Ok(v) => {
            println!("Begin to send start single.");
            v.start();
            println!("Send start single success.");
        },
        Err(e) => {
            println!("Fail to start agent server")
        },
    };
}

#[tauri::command]
fn stop_agent_server(window_state: State<'_, AgentWindowState>) {
    println!("Begin to stop agent server");

    match window_state.agent_server_handler.lock() {
        Ok(v) => {
            println!("Begin to send stop single.");
            v.stop();
            println!("Send stop single success.");
        },
        Err(e) => {
            println!("Fail to stop agent server")
        },
    };
}

struct AgentWindowState {
    agent_server_handler: Mutex<AgentServerHandler>,
    arguments: Arc<AgentArguments>,
    configuration: Arc<AgentConfig>,
}

fn main() -> Result<()> {
    let arguments = AgentArguments::parse();
    let mut log_configuration_file =
        std::fs::File::open(arguments.log_configuration_file.as_deref().unwrap_or(AGNT_LOG_CONFIG_FILE)).expect("Fail to read agnet log configuration file.");
    let mut log_configuration_file_content = String::new();
    log_configuration_file
        .read_to_string(&mut log_configuration_file_content)
        .expect("Fail to read agnet log configuration file");
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
    let configuration = Arc::new(init_configuration(&arguments));
    let system_tray = SystemTray::new();
    let agent_server = AgentServer::new(configuration.clone())?;
    let agent_server_handler = Mutex::new(agent_server.init());
    println!("Begint to initialize GUI");
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![start_agent_server, stop_agent_server])
        .system_tray(system_tray)
        .manage(AgentWindowState {
            agent_server_handler,
            arguments: Arc::new(arguments),
            configuration: configuration.clone(),
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { .. } => {
                std::process::exit(0);
            },
            tauri::WindowEvent::Resized(PhysicalSize { width, height }) => {
                if *width == 0 && *height == 0 {
                    if let Err(e) = event.window().hide() {
                        error!("Fail to hide agent window because of error: {e:#?}");
                    };
                }
            },
            event => {},
        })
        .on_system_tray_event(|app, event| match event {
            tauri::SystemTrayEvent::LeftClick { .. } => {
                let main_window = app.get_window("main").unwrap();
                if let Ok(true) = main_window.is_visible() {
                    if let Err(e) = main_window.hide() {
                        error!("Fail to hide agent window because of error: {e:#?}");
                    };
                } else {
                    if let Err(e) = main_window.show() {
                        error!("Fail to show agent window because of error: {e:#?}");
                    };
                    if let Err(e) = main_window.set_focus() {
                        error!("Fail to forcus agent window because of error: {e:#?}");
                    };
                };
            },
            _ => {
                debug!("System tray event happen.");
            },
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
