#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use clap::Parser;
use common::LogTimer;
use config::{AgentArguments, AgentConfig, AgentLogConfig, UiConfiguration};

use server::AgentServerHandler;
use tauri::{api::dialog::blocking, CustomMenuItem, Manager, PhysicalSize, State, SystemTray, SystemTrayMenu, SystemTrayMenuItem, Window};
use tracing::{debug, error, info, metadata::LevelFilter, Level};
use tracing_subscriber::{fmt::Layer, prelude::__tracing_subscriber_SubscriberExt, Registry};

use crate::server::AgentServer;

pub(crate) mod message {
    pub(crate) mod socks5;
}

pub(crate) mod codec;
pub(crate) mod server;

pub(crate) mod service;

pub(crate) mod config;

const AGENT_LOG_CONFIG_FILE: &str = "ppaass-agent-log.toml";
const DEFAULT_AGENT_CONFIGURATION_FILE: &str = "ppaass-agent.toml";
const EVENT_AGENT_SERVER_START: &str = "agent-server-start-backend-event";
const EVENT_AGENT_SERVER_STOP: &str = "agent-server-stop-backend-event";
const EVENT_AGENT_EXIT: &str = "agent-exit";
const EVENT_AGENT_INITIALIZED: &str = "agent-initialized";
const MAIN_WINDOW_LABEL: &str = "main";

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

#[tauri::command]
fn retrive_agent_configuration(window_state: State<'_, Arc<Mutex<AgentWindowState>>>) -> Result<AgentConfig, String> {
    let window_state = window_state.lock().map_err(|e| e.to_string())?;
    Ok(window_state.configuration.clone())
}

#[tauri::command]
fn start_agent_server(window: Window, window_state: State<'_, Arc<Mutex<AgentWindowState>>>) -> Result<(), String> {
    let window_state = window_state.lock().map_err(|e| e.to_string())?;
    let current_configuration = window_state.configuration.clone();
    println!("The agent configuraiton going to start: {current_configuration:#?}");
    window_state.agent_server_handler.start(current_configuration).map_err(|e| e.to_string())?;
    window.emit_all(EVENT_AGENT_SERVER_START, true).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn stop_agent_server(window: Window, window_state: State<'_, Arc<Mutex<AgentWindowState>>>) -> Result<(), String> {
    debug!("Click to stop agent server button");
    let window_state = window_state.lock().map_err(|e| e.to_string())?;
    window_state.agent_server_handler.stop().map_err(|e| e.to_string())?;
    window.emit_all(EVENT_AGENT_SERVER_STOP, true).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn save_agent_server_config(configuration: UiConfiguration, window_state: State<'_, Arc<Mutex<AgentWindowState>>>) -> Result<(), String> {
    println!("The configuration from ui: {configuration:#?}");
    let mut window_state = window_state.lock().map_err(|e| e.to_string())?;
    if let Some(user_token) = configuration.user_token {
        window_state.configuration.set_user_token(user_token);
    }
    if let Some(proxy_addresses) = configuration.proxy_addresses {
        window_state.configuration.set_proxy_addresses(proxy_addresses);
    }
    if let Some(port) = configuration.port {
        window_state.configuration.set_port(u16::from_str(port.as_str()).map_err(|e| e.to_string())?);
    }
    let configuration_to_save = window_state.configuration.clone();
    let configuration_file_content = toml::to_string_pretty(&configuration_to_save).map_err(|e| e.to_string())?;
    let configuration_file_path = window_state
        .arguments
        .log_configuration_file
        .as_deref()
        .unwrap_or(DEFAULT_AGENT_CONFIGURATION_FILE);
    std::fs::write(configuration_file_path, configuration_file_content).map_err(|e| e.to_string())?;
    println!("The configuration saved: {:#?}", window_state.configuration);
    Ok(())
}

struct AgentWindowState {
    agent_server_handler: AgentServerHandler,
    configuration: AgentConfig,
    arguments: AgentArguments,
}

fn main() -> Result<()> {
    let arguments = AgentArguments::parse();
    let log_configuration_file_content = std::fs::read_to_string(arguments.log_configuration_file.as_deref().unwrap_or(AGENT_LOG_CONFIG_FILE))
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
    let exit_system_tray_menu_item = CustomMenuItem::new(EVENT_AGENT_EXIT.to_string(), "Exit");
    let start_system_tray_menu_item = CustomMenuItem::new(EVENT_AGENT_SERVER_START.to_string(), "Start");
    let stop_system_tray_menu_item = CustomMenuItem::new(EVENT_AGENT_SERVER_STOP.to_string(), "Stop");
    let system_tray_menu = SystemTrayMenu::new()
        .add_item(start_system_tray_menu_item)
        .add_item(stop_system_tray_menu_item)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(exit_system_tray_menu_item);
    let system_tray = SystemTray::new().with_menu(system_tray_menu);
    let agent_server = AgentServer::new()?;
    let agent_server_handler = agent_server.init();
    debug!("Begint to initialize GUI");
    let configuration_for_frontend = configuration.clone();
    tauri::Builder::default()
        .setup(|app| {
            if let Err(e) = app.emit_all(EVENT_AGENT_INITIALIZED, configuration_for_frontend) {
                error!("Fail to initialize frontend because of error:{e:#?}");
            };
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_agent_server,
            stop_agent_server,
            save_agent_server_config,
            retrive_agent_configuration
        ])
        .system_tray(system_tray)
        .manage(Arc::new(Mutex::new(AgentWindowState {
            agent_server_handler,
            configuration,
            arguments,
        })))
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { .. } => {
                info!("Close agent GUI window.");
                std::process::exit(0);
            },
            tauri::WindowEvent::Resized(PhysicalSize { width, height }) => {
                if *width == 0 && *height == 0 {
                    debug!("Going to hide agent GUI on minimize window.");
                    if let Err(e) = event.window().hide() {
                        error!("Fail to hide agent window because of error: {e:#?}");
                    };
                }
            },
            event => {
                debug!("Ignore other window event: {event:#?}");
            },
        })
        .on_system_tray_event(|app, event| {
            let main_window = app.get_window(MAIN_WINDOW_LABEL).unwrap();
            match event {
                tauri::SystemTrayEvent::LeftClick { .. } => {
                    if let Ok(true) = main_window.is_visible() {
                        if let Err(e) = main_window.hide() {
                            error!("Fail to hide agent window because of error: {e:#?}");
                        };
                    } else {
                        if let Err(e) = main_window.show() {
                            error!("Fail to show agent window because of error: {e:#?}");
                        };
                    };
                },
                tauri::SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                    EVENT_AGENT_EXIT => {
                        info!("Close agent GUI window.");
                        std::process::exit(0);
                    },
                    EVENT_AGENT_SERVER_START => {
                        let window_state = main_window.state::<Arc<Mutex<AgentWindowState>>>().clone();
                        if let Ok(window_state) = window_state.lock() {
                            let current_configuration = window_state.configuration.clone();
                            if let Err(e) = window_state.agent_server_handler.start(current_configuration) {
                                blocking::message(Some(&main_window), "Error", e.to_string());
                                error!("Fail to start agent server because of exception: {e:#?}");
                            };
                            if let Err(e) = main_window.emit_all(EVENT_AGENT_SERVER_START, true) {
                                error!("Fail to send start single to agent server ui because of error: {e:#?}");
                            };
                        };
                    },
                    EVENT_AGENT_SERVER_STOP => {
                        let window_state = main_window.state::<Arc<Mutex<AgentWindowState>>>().clone();
                        if let Ok(window_state) = window_state.lock() {
                            if let Err(e) = window_state.agent_server_handler.stop() {
                                blocking::message(Some(&main_window), "Error", e.to_string());
                                error!("Fail to stop agent server because of exception: {e:#?}");
                            };
                            if let Err(e) = main_window.emit_all(EVENT_AGENT_SERVER_STOP, true) {
                                error!("Fail to send stop single to agent server ui because of error: {e:#?}");
                            };
                        };
                    },
                    unknown => {
                        debug!("Nothing to do because of unknown system tray menu item: {}", unknown)
                    },
                },
                _ => {
                    debug!("System tray event happen.");
                },
            }
        })
        .run(tauri::generate_context!())
        .expect("Error happen while create agent GUI.");
    Ok(())
}
